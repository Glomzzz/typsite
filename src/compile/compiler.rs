//! Build orchestration. Source discovery, export caching, graph resolution,
//! rendering and publication have separate responsibilities and failure modes.

use super::cache::{Export, Manifest, Publication, fingerprint};
use super::graph::ResolvedSite;
use super::options::CompileOptions;
use super::output::{LOCK, OutputPlan};
use super::project::{ProjectPaths, Snapshot, digest, managed_path, scan};
use super::registry::KeyRegistry;
use super::report::{BuildReport, Stage};
use super::typst::{CommandTypst, ExportRequest, TypstRunner};
use crate::config::TypsiteConfig;
use crate::pass::pass_pure;
use anyhow::{Context, Result, ensure};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Compiler {
    paths: ProjectPaths,
    options: CompileOptions,
    runner: Arc<dyn TypstRunner>,
    engine_identity: String,
}

impl Compiler {
    pub fn new(
        options: CompileOptions,
        cache: PathBuf,
        config: PathBuf,
        input: PathBuf,
        output: PathBuf,
        packages: Option<PathBuf>,
        typst: String,
    ) -> Result<Self> {
        Self::with_runner(
            ProjectPaths::new(input, config, cache, output, packages)?,
            options,
            Arc::new(CommandTypst::new(typst)),
        )
    }

    pub fn with_runner(
        paths: ProjectPaths,
        options: CompileOptions,
        runner: Arc<dyn TypstRunner>,
    ) -> Result<Self> {
        paths.validate()?;
        Ok(Self {
            paths,
            options,
            runner,
            engine_identity: digest(
                &std::env::current_exe().context("Cannot locate build engine")?,
            )?,
        })
    }

    pub fn output_path(&self) -> &Path {
        &self.paths.output
    }

    pub fn clean(&self) -> Result<()> {
        self.paths.validate()?;
        super::clean::clean_roots(
            &self.paths.cache,
            &self.paths.output,
            &[&self.paths.input, &self.paths.config],
            false,
        )
    }

    pub async fn watch(self, host: String, port: u16) -> Result<()> {
        super::watch::watch(self, host, port).await
    }

    pub fn compile(&self) -> Result<BuildReport> {
        self.paths.validate()?;
        let _cache_lock = lock(&self.paths.cache)?;
        let _output_lock = lock(&self.paths.output)?;
        let mut report = BuildReport::default();
        let paths = &self.paths;
        let exports_root = paths.cache.join("exports");
        let config = TypsiteConfig::load_with_options(
            &paths.config,
            &paths.input,
            &exports_root,
            self.options,
        )
        .context("Cannot load project configuration")?;
        let input = scan(&paths.input)?;
        let configuration = scan(&paths.config)?;
        let packages = paths
            .packages
            .as_deref()
            .map(scan)
            .transpose()?
            .unwrap_or_default();
        let sources: Snapshot = input
            .iter()
            .filter(|(path, _)| is_typst(path) && !is_library(path, &config))
            .map(|(path, hash)| (path.clone(), hash.clone()))
            .collect();
        let fonts: Snapshot = configuration
            .iter()
            .filter(|(path, _)| path.starts_with("assets/fonts"))
            .map(|(path, hash)| (path.clone(), hash.clone()))
            .collect();
        let identity = self.runner.identity()?;
        let export_key = fingerprint(&(
            1_u32,
            &self.engine_identity,
            &identity,
            &paths.input,
            &paths.config,
            &paths.packages,
            &packages,
            &fonts,
        ))?;
        let mut cache = Manifest::load(&paths.cache);
        let previous_manifest = fingerprint(&cache)?;
        // Hash known external dependencies before exporting, not just after.
        // New dependencies discovered by Typst require a verification export.
        let mut expected: Snapshot = cache
            .exports
            .values()
            .flat_map(|entry| entry.dependencies.keys())
            .filter_map(|path| digest(path).ok().map(|hash| (path.clone(), hash)))
            .collect();
        for (root, snapshot) in [(&paths.input, &input), (&paths.config, &configuration)] {
            for (relative, hash) in snapshot {
                expected.insert(fs::canonicalize(root.join(relative))?, hash.clone());
            }
        }
        if let Some(root) = &paths.packages {
            for (relative, hash) in &packages {
                expected.insert(fs::canonicalize(root.join(relative))?, hash.clone());
            }
        }
        let mut prepared = false;
        if cache.export_key != export_key {
            self.runner.prepare(paths.packages.as_deref())?;
            prepared = true;
            cache.exports.clear();
            cache.export_key = export_key;
        }
        cache
            .exports
            .retain(|source, _| sources.contains_key(source));
        for (relative, source_hash) in &sources {
            let artifact = managed_path(
                &paths.cache,
                &Path::new("exports").join(relative).with_extension("html"),
            )?;
            if cache
                .exports
                .get(relative)
                .is_some_and(|entry| entry.is_current(source_hash, &artifact))
            {
                continue;
            }
            if !prepared {
                // A local package namespace can disappear independently of the
                // build cache. Repair it before retrying an invalid export.
                self.runner.prepare(paths.packages.as_deref())?;
                prepared = true;
            }
            // Never use last build's HTML after a failed export of this source.
            cache.exports.remove(relative);
            let source = paths.input.join(relative);
            let result = (|| {
                let mut before = expected.clone();
                for _ in 0..3 {
                    let pending = crate::util::atomic::PendingFile::new(&artifact)?;
                    let mut dependencies = self.runner.export(&ExportRequest {
                        root: &paths.input,
                        input: &source,
                        output: pending.path(),
                        font_path: &paths.config.join("assets/fonts"),
                    })?;
                    dependencies.push(source.clone());
                    let dependencies = dependencies
                        .into_iter()
                        .map(|path| {
                            let path = if path.is_absolute() {
                                path
                            } else {
                                paths.input.join(path)
                            };
                            let path = fs::canonicalize(&path).with_context(|| {
                                format!("Cannot resolve Typst dependency {}", path.display())
                            })?;
                            Ok((path.clone(), digest(&path)?))
                        })
                        .collect::<Result<Snapshot>>()?;
                    ensure!(
                        dependencies.get(&fs::canonicalize(&source)?) == Some(source_hash),
                        "Source changed during export: {}",
                        source.display()
                    );
                    if dependencies
                        .iter()
                        .all(|(path, hash)| before.get(path) == Some(hash))
                    {
                        let html_hash = digest(pending.path())?;
                        pending.commit(&artifact)?;
                        return Ok::<_, anyhow::Error>(Export {
                            source_hash: source_hash.clone(),
                            html_hash,
                            dependencies,
                        });
                    }
                    // The first export discovered files we could not fingerprint
                    // beforehand, or files changed while Typst was reading them.
                    before = dependencies;
                }
                anyhow::bail!("Typst dependencies did not stabilize during export")
            })();
            match result {
                Ok(entry) => {
                    cache.exports.insert(relative.clone(), entry);
                    report.exported.push(relative.clone());
                }
                Err(error) => {
                    report.error(Stage::Export, Some(relative.clone()), format!("{error:#}"))
                }
            }
        }
        // Export artifacts are durable independently of publication. Successful
        // exports survive a render/output failure; failed sources remain misses.
        if fingerprint(&cache)? != previous_manifest {
            cache.save(&paths.cache)?;
        }
        if !report.is_success() {
            return Ok(report);
        }
        let build_key = fingerprint(&(
            env!("CARGO_PKG_VERSION"),
            &self.engine_identity,
            &cache.export_key,
            &input,
            &configuration,
            &self.options,
            &cache.exports,
        ))?;
        if let Some(published) = &cache.published
            && published.key == build_key
            && OutputPlan::is_current(&paths.output, &published.outputs)?
        {
            return Ok(report);
        }
        let mut registry = KeyRegistry::new();
        let mut known = BTreeMap::new();
        for relative in sources.keys() {
            let source = paths.input.join(relative);
            let slug = config.path_to_slug(&source)?;
            ensure!(
                known.insert(slug.clone(), source.clone()).is_none(),
                "Duplicate article slug {slug}"
            );
            registry.register_article_path(&config, &source)?;
        }
        let mut articles = HashMap::new();
        for relative in sources.keys() {
            let source = paths.input.join(relative);
            let (slug, source) = registry.register_article_path(&config, &source)?;
            let html = fs::read_to_string(managed_path(
                &paths.cache,
                &Path::new("exports").join(relative).with_extension("html"),
            )?)?;
            match pass_pure(&config, &registry, source, slug.clone(), None, &html) {
                Ok(article) => {
                    articles.insert(slug, article);
                }
                Err(error) => report.error(Stage::Parse, Some(relative.clone()), error),
            }
        }
        if !report.is_success() {
            return Ok(report);
        }
        let site = match ResolvedSite::resolve(&config, articles) {
            Ok(site) => site,
            Err(error) => {
                report.error(Stage::Resolve, None, format!("{error:#}"));
                return Ok(report);
            }
        };
        let pages = super::render::render(&config, &site, &mut report);
        if !report.is_success() {
            return Ok(report);
        }
        let mut output = OutputPlan::default();
        for (source, content) in pages {
            let relative = source.strip_prefix(&paths.input)?.with_extension("html");
            output.insert(relative, content.into_bytes(), source.display().to_string())?;
        }
        for relative in input
            .keys()
            .filter(|path| !is_typst(path) && !is_library(path, &config))
        {
            output.insert(
                relative.clone(),
                fs::read(paths.input.join(relative))?,
                format!("source asset {}", relative.display()),
            )?;
        }
        for relative in configuration
            .keys()
            .filter_map(|path| path.strip_prefix("assets").ok())
            .filter(|path| path.extension().is_none_or(|ext| ext != "html"))
        {
            output.insert(
                relative.to_owned(),
                fs::read(paths.config.join("assets").join(relative))?,
                format!("config asset {}", relative.display()),
            )?;
        }
        for generated in super::site_output::generate_site_outputs(&site.articles, &config.options)?
        {
            output.insert(
                generated.path,
                generated.content.into_bytes(),
                "generated site file",
            )?;
        }
        // Do not publish a mixture of two source/config generations if an edit
        // raced with a long export. The next normal build will retry safely.
        if scan(&paths.input)? != input
            || scan(&paths.config)? != configuration
            || paths
                .packages
                .as_deref()
                .map(scan)
                .transpose()?
                .unwrap_or_default()
                != packages
            || sources.iter().any(|(relative, hash)| {
                !cache.exports[relative]
                    .is_current(hash, &exports_root.join(relative).with_extension("html"))
            })
        {
            // Never retain exports whose post-export hashes may describe a
            // different generation than the bytes Typst read during the build.
            cache.exports.clear();
            cache.save(&paths.cache)?;
            report.error(
                Stage::Discover,
                None,
                "Inputs changed during the build; retrying on the next build",
            );
            return Ok(report);
        }
        match output.publish(&paths.output, &mut report) {
            Ok(outputs) => {
                cache.published = Some(Publication {
                    key: build_key,
                    outputs,
                });
                if let Err(error) = cache.save(&paths.cache) {
                    report.error(Stage::Cache, None, format!("{error:#}"));
                }
            }
            Err(error) => report.error(Stage::Publish, None, format!("{error:#}")),
        }
        Ok(report)
    }
}

pub(super) fn lock(root: &Path) -> Result<File> {
    fs::create_dir_all(root)
        .with_context(|| format!("Cannot create managed directory {}", root.display()))?;
    let path = managed_path(root, Path::new(LOCK))?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock().with_context(|| {
        format!(
            "Project directory is already being built: {}",
            root.display()
        )
    })?;
    Ok(file)
}

fn is_typst(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "typ")
}

fn is_library(path: &Path, config: &TypsiteConfig<'_>) -> bool {
    let path = path.to_string_lossy().replace('\\', "/");
    let library = &config.options.project.typst_lib;
    library.files.contains(&path) || library.dirs.iter().any(|prefix| path.starts_with(prefix))
}
