//! Build orchestration. Source discovery, export caching, graph resolution,
//! rendering and publication have separate responsibilities and failure modes.

use super::cache::{Manifest, Publication, fingerprint};
use super::graph::ResolvedSite;
use super::options::CompileOptions;
use super::output::{LOCK, OutputPlan};
use super::project::{ProjectPaths, Snapshot, digest, managed_path, scan};
use super::registry::KeyRegistry;
use super::report::{BuildReport, Stage};
use super::typst::{CommandTypst, TypstRunner};
use crate::config::TypsiteConfig;
use crate::pass::pass_pure;
use anyhow::{Context, Result, ensure};
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, File, OpenOptions};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Compiler {
    paths: ProjectPaths,
    options: CompileOptions,
    runner: Arc<dyn TypstRunner>,
    engine_identity: String,
    export_jobs: Option<NonZeroUsize>,
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
        let engine_identity = env!("TYPSITE_ENGINE_ID").to_owned();
        Ok(Self {
            paths,
            options,
            runner,
            export_jobs: None,
            engine_identity,
        })
    }

    /// Bound simultaneous article exports. Scheduling is not an input to the
    /// generated site, so changing this limit does not invalidate warm caches.
    pub fn with_jobs(mut self, jobs: NonZeroUsize) -> Self {
        self.export_jobs = Some(jobs);
        self
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
        // Typst --version is a subprocess on the CLI path. Overlap it with
        // filesystem reads without initializing Rayon on a small warm project.
        let (input, configuration, packages, identity, mut cache, exports_valid, outputs_valid) =
            std::thread::scope(|scope| -> Result<_> {
                let identity = scope.spawn(|| self.runner.identity());
                let input = scan(&paths.input)?;
                let configuration = scan(&paths.config)?;
                let packages = paths
                    .packages
                    .as_deref()
                    .map(scan)
                    .transpose()?
                    .unwrap_or_default();
                let cache = Manifest::load(&paths.cache);
                let exports_valid = cache.published.is_some()
                    && super::cache::exports_current(
                        &cache.exports,
                        &input,
                        &exports_root,
                        &paths.input,
                    )?;
                let outputs_valid = if let Some(published) = &cache.published {
                    OutputPlan::is_current(&paths.output, &published.outputs)?
                } else {
                    false
                };
                let identity = identity
                    .join()
                    .map_err(|_| anyhow::anyhow!("Typst identity worker panicked"))??;
                Ok((
                    input,
                    configuration,
                    packages,
                    identity,
                    cache,
                    exports_valid,
                    outputs_valid,
                ))
            })?;
        let input_key = fingerprint(&(
            env!("CARGO_PKG_VERSION"),
            &self.engine_identity,
            &identity,
            &paths.input,
            &paths.config,
            &paths.packages,
            &self.options,
            &input,
            &configuration,
            &packages,
        ))?;
        // A verified no-op needs no template/YAML/theme parsing. The complete
        // configuration snapshot is still checked, including unused resources;
        // any changed resource forces normal strict validation below.
        if let Some(published) = &cache.published
            && published.inputs == input_key
            && published.sources.len() == cache.exports.len()
            && published
                .sources
                .iter()
                .all(|source| cache.exports.contains_key(source))
            && exports_valid
            && outputs_valid
        {
            return Ok(report);
        }
        let previous_manifest = fingerprint(&cache)?;
        let highlight_inputs: Snapshot = configuration
            .iter()
            .filter(|(path, _)| path.starts_with("themes") || path.starts_with("syntaxes"))
            .map(|(path, hash)| (path.clone(), hash.clone()))
            .collect();
        let highlight_key =
            fingerprint(&(&self.engine_identity, &paths.config, &highlight_inputs))?;
        let config = TypsiteConfig::load_cached(
            &paths.config,
            &paths.input,
            &exports_root,
            self.options,
            &paths.cache,
            &highlight_key,
        )
        .context("Cannot load project configuration")?;
        let site_outputs = super::site_output::SiteOutputs::new(&config.options)?;
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
        // Hash known external dependencies before exporting, not just after.
        // New dependencies discovered by Typst require a verification export.
        let mut expected = Snapshot::new();
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
        // Shared package/import files can be dependencies of every article.
        // Fingerprint each external path once, rather than once per importer.
        let external: BTreeSet<_> = cache
            .exports
            .values()
            .flat_map(|entry| entry.dependencies.keys())
            .filter(|path| !expected.contains_key(*path))
            .cloned()
            .collect();
        let external: Vec<_> = external
            .par_iter()
            .filter_map(|path| digest(path).ok().map(|hash| (path.clone(), hash)))
            .collect();
        expected.extend(external);
        let identity_changed = cache.export_key != export_key;
        if identity_changed {
            cache.exports.clear();
            cache.export_key = export_key;
        }
        cache
            .exports
            .retain(|source, _| sources.contains_key(source));
        // Preflight all targets before any workers write. Preserve source order
        // when folding results, irrespective of worker completion order.
        let candidates = {
            let mut names = OutputPlan::default();
            sources
                .iter()
                .map(|(relative, hash)| {
                    let target = relative.with_extension("html");
                    // Two workers must never target the same artifact, including
                    // portable case-insensitive or file/directory collisions.
                    names.insert(target.clone(), Vec::new(), relative.display().to_string())?;
                    let artifact = managed_path(&paths.cache, &Path::new("exports").join(target))?;
                    Ok((relative, hash, artifact))
                })
                .collect::<Result<Vec<_>>>()?
        };
        let validity: Vec<_> = candidates
            .par_iter()
            .map(|(relative, hash, artifact)| {
                cache
                    .exports
                    .get(*relative)
                    .is_some_and(|entry| entry.matches_snapshot(hash, artifact, &expected))
            })
            .collect();
        let dirty: Vec<_> = candidates
            .into_iter()
            .zip(validity)
            .filter_map(|(candidate, current)| (!current).then_some(candidate))
            .collect();
        if identity_changed || !dirty.is_empty() {
            // Includes repairing a deleted package namespace even if the build's
            // export identity is unchanged. Never race installers with exports.
            for root in self.runner.prepare(paths.packages.as_deref())? {
                let root =
                    fs::canonicalize(root).context("Cannot resolve prepared package root")?;
                for (relative, hash) in scan(&root)? {
                    expected.insert(fs::canonicalize(root.join(relative))?, hash);
                }
            }
        }
        // Never retain validity for a failed export. Successful siblings still
        // reach the independent export checkpoint after every worker has joined.
        for (relative, _, _) in &dirty {
            cache.exports.remove(*relative);
        }
        let concurrent = self
            .export_jobs
            .map_or_else(cpu_budget, NonZeroUsize::get)
            .min(dirty.len())
            .max(1);
        let typst_jobs = NonZeroUsize::new((cpu_budget() / concurrent).max(1)).unwrap();
        let results = super::export::export_batch(
            self.runner.as_ref(),
            paths,
            &dirty,
            &expected,
            concurrent,
            typst_jobs,
        )?;
        for ((relative, _, _), result) in dirty.into_iter().zip(results) {
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
        // Exports are checkpointed independently of publication. Successful
        // exports survive ordinary render/output failures; power-loss damage to
        // this disposable cache is detected by checksums and artifact hashes.
        if fingerprint(&cache)? != previous_manifest {
            cache.save(&paths.cache)?;
        }
        if !report.is_success() {
            return Ok(report);
        }
        let build_key = fingerprint(&(&input_key, &cache.export_key, &cache.exports))?;
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
        let parse_jobs = sources
            .keys()
            .map(|relative| {
                let source = paths.input.join(relative);
                let slug = config.path_to_slug(&source)?;
                Ok((
                    relative,
                    registry.slug(&slug).context("Unregistered article slug")?,
                    registry.path(&slug).context("Unregistered article path")?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        // The registry/config are immutable after registration. Each parse owns
        // its buffers and returns a result; diagnostics are folded in slug order.
        let parsed: Vec<_> = parse_jobs
            .par_iter()
            .map(|(relative, slug, source)| {
                let html = fs::read_to_string(managed_path(
                    &paths.cache,
                    &Path::new("exports").join(relative).with_extension("html"),
                )?)?;
                Ok(pass_pure(
                    &config,
                    &registry,
                    source.clone(),
                    slug.clone(),
                    None,
                    &html,
                ))
            })
            .collect::<Vec<Result<_>>>();
        let mut articles = HashMap::new();
        for ((relative, slug, _), result) in parse_jobs.into_iter().zip(parsed) {
            match result? {
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
        let feed_link = site_outputs.discovery_link();
        let pages = super::render::render(&config, &site, &mut report, feed_link.as_deref());
        if !report.is_success() {
            return Ok(report);
        }
        let generated = site_outputs.generate(&site.articles, &pages)?;
        let mut output = OutputPlan::default();
        for page in pages {
            let relative = page.path.strip_prefix(&paths.input)?.with_extension("html");
            output.insert(
                relative,
                page.html.into_bytes(),
                page.path.display().to_string(),
            )?;
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
        for generated in generated {
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
            || !super::cache::exports_current(&cache.exports, &input, &exports_root, &paths.input)?
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
                    inputs: input_key,
                    sources: sources.keys().cloned().collect(),
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

fn cpu_budget() -> usize {
    std::thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(rayon::current_num_threads())
        .max(1)
}

fn is_typst(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "typ")
}

fn is_library(path: &Path, config: &TypsiteConfig<'_>) -> bool {
    config.options.project.typst_lib.contains(path)
}
