//! Native book compilation. A book owns one file, never the HTML output set.

use super::cache::fingerprint;
use super::compiler::lock;
use super::options::ProjOptions;
use super::output::OWNERSHIP;
use super::project::{Snapshot, digest, managed_path, resolve, scan, validate_relative};
use super::typst::{CommandTypst, PdfRequest, TypstRunner};
use crate::util::atomic::{PendingFile, write, write_cache};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use url::Url;

const CACHE_MARKER: &[u8] = b"TYPSITE-PDF-CACHE-1\n";

pub(super) fn owns_cache_root(root: &Path) -> bool {
    fs::read(root.join("pdf/format")).is_ok_and(|bytes| bytes == CACHE_MARKER)
}

#[derive(Debug, Clone, Serialize)]
pub struct PdfOptions {
    /// Source-root-relative article reference (normally a .typ path).
    pub entry: String,
    pub appendices: Vec<String>,
    /// Source-root-relative module exporting `book-style(body)`.
    pub template: Option<String>,
    pub title: Option<String>,
    pub pretty_url: bool,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            entry: "index.typ".into(),
            appendices: Vec::new(),
            template: None,
            title: None,
            pretty_url: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct PdfReport {
    pub compiled: bool,
    pub written: bool,
}

pub struct PdfCompiler {
    input: PathBuf,
    config: PathBuf,
    cache: PathBuf,
    output: PathBuf,
    packages: Option<PathBuf>,
    options: PdfOptions,
    runner: Arc<dyn TypstRunner>,
    jobs: NonZeroUsize,
}

#[derive(Serialize, Deserialize)]
struct Cache {
    version: u32,
    key: String,
    dependencies: Snapshot,
    pdf_hash: String,
}

impl PdfCompiler {
    pub fn new(
        input: PathBuf,
        config: PathBuf,
        cache: PathBuf,
        output: PathBuf,
        packages: Option<PathBuf>,
        options: PdfOptions,
        typst: String,
    ) -> Result<Self> {
        Self::with_runner(
            input,
            config,
            cache,
            output,
            packages,
            options,
            Arc::new(CommandTypst::new(typst)),
        )
    }

    pub fn with_runner(
        input: PathBuf,
        config: PathBuf,
        cache: PathBuf,
        output: PathBuf,
        packages: Option<PathBuf>,
        options: PdfOptions,
        runner: Arc<dyn TypstRunner>,
    ) -> Result<Self> {
        let output = std::path::absolute(output)?;
        reject_link(&output)?;
        let output = resolve(output.parent().context("PDF output has no parent")?)?
            .join(output.file_name().context("PDF output has no filename")?);
        let compiler = Self {
            input: resolve(&input)?,
            config: resolve(&config)?,
            cache: resolve(&cache)?,
            output,
            packages: packages.map(|path| resolve(&path)).transpose()?,
            options,
            runner,
            jobs: std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN),
        };
        compiler.validate()?;
        Ok(compiler)
    }

    pub fn with_jobs(mut self, jobs: NonZeroUsize) -> Self {
        self.jobs = jobs;
        self
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.input.is_dir(),
            "Typst input directory does not exist: {}",
            self.input.display()
        );
        ensure!(
            self.config.is_dir(),
            "Configuration directory does not exist: {}",
            self.config.display()
        );
        let roots = [&self.input, &self.config, &self.cache];
        for (index, root) in roots.iter().enumerate() {
            ensure!(
                resolve(root)? == **root,
                "PDF project root changed through a symbolic link: {}",
                root.display()
            );
            for other in &roots[index + 1..] {
                ensure!(
                    !root.starts_with(other) && !other.starts_with(root),
                    "PDF source, configuration, and cache roots must not overlap"
                );
            }
            ensure!(
                !self.output.starts_with(root) && !root.starts_with(&self.output),
                "PDF output must not overlap source, configuration, or cache: {}",
                self.output.display()
            );
        }
        if let Some(packages) = &self.packages {
            ensure!(
                packages.is_dir() && resolve(packages)? == *packages,
                "Invalid PDF packages directory"
            );
            ensure!(
                !packages.starts_with(&self.cache)
                    && !self.cache.starts_with(packages)
                    && !self.output.starts_with(packages),
                "PDF packages overlap a writable path"
            );
        }
        ensure!(
            self.output
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf")),
            "PDF output must be a .pdf file"
        );
        ensure!(!self.output.is_dir(), "PDF output is a directory");
        reject_link(&self.output)?;
        ensure!(
            resolve(self.output.parent().unwrap())? == self.output.parent().unwrap(),
            "PDF output parent changed through a symbolic link"
        );
        self.check_html_ownership()
    }

    fn check_html_ownership(&self) -> Result<()> {
        // A standalone book must not overwrite an artifact owned by an HTML
        // build. It never modifies that build's complete ownership journal.
        #[derive(Deserialize)]
        struct Ownership {
            files: BTreeSet<PathBuf>,
        }
        for parent in self.output.parent().unwrap().ancestors() {
            let journal = parent.join(OWNERSHIP);
            if !journal.exists() {
                continue;
            }
            reject_link(&journal)?;
            let ownership: Ownership = serde_json::from_slice(&fs::read(&journal)?)
                .context("Invalid HTML ownership journal; cannot safely publish PDF here")?;
            let relative = self
                .output
                .strip_prefix(parent)?
                .to_string_lossy()
                .replace('\\', "/")
                .to_lowercase();
            ensure!(
                !ownership.files.iter().any(|path| path
                    .to_string_lossy()
                    .replace('\\', "/")
                    .to_lowercase()
                    == relative),
                "PDF output is owned by an HTML build: {}; choose a separate book output",
                self.output.display()
            );
        }
        Ok(())
    }

    pub fn compile(&self) -> Result<PdfReport> {
        self.validate()?;
        let _cache_lock = lock(&self.cache)?;
        let _output_lock = lock(self.output.parent().unwrap())?;
        self.validate()?;
        let installed = self.runner.prepare(self.packages.as_deref())?;
        let mut roots = vec![self.input.clone(), self.config.clone()];
        roots.extend(self.packages.iter().cloned());
        roots.extend(installed);
        let mut roots: BTreeMap<_, _> = roots.into_iter().map(|root| (root, false)).collect();
        if let Some(fonts) = std::env::var_os("TYPST_FONT_PATHS") {
            for root in std::env::split_paths(&fonts).filter(|path| path.is_dir()) {
                // Explicit font projections (including Nix profiles) may link
                // outside their directory. Source/package trees remain strict.
                roots.entry(resolve(&root)?).or_insert(true);
            }
        }
        let before = snapshots(&roots)?;
        let options = ProjOptions::load(&self.config)?;
        let input = before.get(&self.input).context("Missing input snapshot")?;
        let driver = self.driver(input, &options)?;
        let identity = self.runner.identity()?;
        let key = fingerprint(&(
            1_u32,
            env!("TYPSITE_ENGINE_ID"),
            &identity,
            &self.input,
            &self.config,
            &self.packages,
            &self.options,
            &driver,
            &before,
        ))?;
        let cache_dir = managed_path(&self.cache, Path::new("pdf"))?;
        fs::create_dir_all(&cache_dir)?;
        let marker = managed_path(&cache_dir, Path::new("format"))?;
        if !fs::read(&marker).is_ok_and(|bytes| bytes == CACHE_MARKER) {
            write_cache(&marker, CACHE_MARKER)?;
        }
        let name = fingerprint(&self.output)?;
        let manifest = managed_path(&cache_dir, Path::new(&format!("{name}.json")))?;
        let artifact = managed_path(&cache_dir, Path::new(&format!("{name}.pdf")))?;
        let cached = fs::read(&manifest)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Cache>(&bytes).ok())
            .filter(|cache| cache.version == 1);
        let mut report = PdfReport::default();
        if let Some(cache) = &cached
            && cache.key == key
            && current(&cache.dependencies)
            && digest(&artifact).is_ok_and(|hash| hash == cache.pdf_hash)
        {
            ensure!(
                snapshots(&roots)? == before && current(&cache.dependencies),
                "Inputs changed during PDF build; retry"
            );
            if digest(&self.output).is_ok_and(|hash| hash == cache.pdf_hash) {
                return Ok(report);
            }
            self.publish(&fs::read(&artifact)?, &mut report)?;
            return Ok(report);
        }
        let mut expected = absolute_snapshot(&before)?;
        if let Some(cache) = &cached {
            for path in cache.dependencies.keys() {
                if let Ok(hash) = digest(path) {
                    expected.insert(path.clone(), hash);
                }
            }
        }
        for _ in 0..3 {
            let pending = PendingFile::new(&artifact)?;
            let dependencies = self.runner.pdf(&PdfRequest {
                root: &self.input,
                driver: &driver,
                output: pending.path(),
                font_path: &self.config.join("assets/fonts"),
                jobs: self.jobs,
            })?;
            report.compiled = true;
            ensure!(
                snapshots(&roots)? == before,
                "Inputs changed during PDF compilation; last successful PDF retained, retry the build"
            );
            let mut after = Snapshot::new();
            for dependency in dependencies {
                let path = if dependency.is_absolute() {
                    dependency
                } else {
                    self.input.join(dependency)
                };
                let path = fs::canonicalize(&path)
                    .with_context(|| format!("Cannot resolve PDF dependency {}", path.display()))?;
                after.insert(path.clone(), digest(&path)?);
            }
            if !after
                .iter()
                .all(|(path, hash)| expected.get(path) == Some(hash))
            {
                // Unknown imports require a verification compilation; do not
                // mark bytes current based only on hashes taken after reading.
                expected.extend(after);
                continue;
            }
            let bytes = fs::read(pending.path())?;
            ensure!(
                bytes.starts_with(b"%PDF-"),
                "Typst did not produce a PDF document"
            );
            let pdf_hash = blake3::hash(&bytes).to_hex().to_string();
            ensure!(
                current(&after) && snapshots(&roots)? == before,
                "Inputs changed before PDF publication; retry"
            );
            self.validate()?;
            pending.commit_cache(&artifact)?;
            self.publish(&bytes, &mut report)?;
            let cache = Cache {
                version: 1,
                key,
                dependencies: after,
                pdf_hash,
            };
            write_cache(&manifest, &serde_json::to_vec(&cache)?)?;
            return Ok(report);
        }
        anyhow::bail!("PDF dependencies did not stabilize; last successful PDF retained")
    }

    fn publish(&self, bytes: &[u8], report: &mut PdfReport) -> Result<()> {
        self.validate()?;
        if fs::read(&self.output).is_ok_and(|old| old == bytes) {
            return Ok(());
        }
        write(&self.output, bytes).context("Cannot publish PDF")?;
        report.written = true;
        Ok(())
    }

    fn driver(&self, input: &Snapshot, options: &ProjOptions) -> Result<String> {
        let mut pages = BTreeMap::new();
        let mut portable = BTreeSet::new();
        for path in input.keys().filter(|path| {
            path.extension().is_some_and(|ext| ext == "typ") && !options.typst_lib.contains(path)
        }) {
            validate_relative(path)?;
            let slug = format!(
                "/{}",
                path.with_extension("").to_string_lossy().replace('\\', "/")
            );
            ensure!(
                portable.insert(slug.to_lowercase()),
                "PDF article paths collide on a case-insensitive filesystem: {slug}"
            );
            pages.insert(
                slug,
                format!("/{}", path.to_string_lossy().replace('\\', "/")),
            );
        }
        let entry = source_reference(&self.options.entry)?;
        let appendices = self
            .options
            .appendices
            .iter()
            .map(|reference| source_reference(reference))
            .collect::<Result<Vec<_>>>()?;
        // Explicit roots may be standalone print pages in typst_lib. Libraries
        // remain excluded from implicit discovery and from normal HTML builds.
        for slug in std::iter::once(&entry).chain(&appendices) {
            if pages.contains_key(slug) {
                continue;
            }
            let path = format!("{}.typ", slug.trim_start_matches('/'));
            ensure!(
                input.contains_key(Path::new(&path)),
                "PDF root article not found: {slug}"
            );
            ensure!(
                portable.insert(slug.to_lowercase()),
                "PDF root paths collide on a case-insensitive filesystem: {slug}"
            );
            pages.insert(slug.clone(), format!("/{path}"));
        }
        let mut driver = String::from(
            "#import \"@local/typsite:0.1.0\": pdf-book\n#set page(paper: \"a4\", margin: 2cm, numbering: \"1\")\n#set heading(numbering: \"1.1\")\n",
        );
        if let Some(template) = &self.options.template {
            let relative = source_path(template)?;
            ensure!(
                input.contains_key(&relative),
                "PDF template not found inside source root: {template}"
            );
            driver.push_str(&format!(
                "#import {}: book-style\n#show: book-style\n",
                typst_string(&format!(
                    "/{}",
                    relative.to_string_lossy().replace('\\', "/")
                ))
            ));
        }
        driver.push_str("#pdf-book(\n  pages: (\n");
        for (slug, path) in &pages {
            driver.push_str(&format!(
                "    {}: () => include {},\n",
                typst_string(slug),
                typst_string(path)
            ));
        }
        driver.push_str(&format!(
            "  ),\n  entry: {},\n  appendices: {},\n",
            typst_string(&entry),
            json_value(&appendices)?
        ));
        let title = self.options.title.as_deref().unwrap_or(&options.site.title);
        let mut urls = BTreeMap::new();
        if !options.site.base_url.trim().is_empty() {
            let mut base = Url::parse(options.site.base_url.trim())
                .context("site.base_url must be an absolute HTTP(S) URL")?;
            ensure!(
                matches!(base.scheme(), "http" | "https")
                    && base.host_str().is_some()
                    && base.username().is_empty()
                    && base.password().is_none()
                    && base.query().is_none()
                    && base.fragment().is_none(),
                "site.base_url must be an absolute HTTP(S) URL without credentials, query, or fragment"
            );
            if !base.path().ends_with('/') {
                base.set_path(&format!("{}/", base.path()));
            }
            for slug in pages.keys() {
                let mut url = base.clone();
                let path = if self.options.pretty_url {
                    slug.trim_start_matches('/').to_owned()
                } else {
                    format!("{}.html", slug.trim_start_matches('/'))
                };
                url.path_segments_mut()
                    .map_err(|_| anyhow::anyhow!("Invalid site.base_url"))?
                    .pop_if_empty()
                    .extend(path.split('/'));
                urls.insert(slug, url.to_string());
            }
        }
        let defaults: BTreeMap<_, _> = options.default_metadata.content.default.iter().collect();
        let root_prefix = format!(
            "{}/",
            self.input.file_name().unwrap_or_default().to_string_lossy()
        );
        let parent = options
            .default_metadata
            .graph
            .parent
            .as_deref()
            .map(|parent| source_reference(parent.strip_prefix(&root_prefix).unwrap_or(parent)))
            .transpose()?;
        driver.push_str(&format!(
            "  title: {},\n  defaults: {},\n  default-parent: {},\n  urls: {},\n)\n",
            typst_string(title),
            json_value(&defaults)?,
            json_value(&parent)?,
            json_value(&urls)?
        ));
        Ok(driver)
    }
}

fn snapshots(roots: &BTreeMap<PathBuf, bool>) -> Result<BTreeMap<PathBuf, Snapshot>> {
    roots
        .iter()
        .map(|(root, fonts)| {
            Ok((
                root.clone(),
                if *fonts {
                    scan_fonts(root)?
                } else {
                    scan(root)?
                },
            ))
        })
        .collect()
}

fn scan_fonts(root: &Path) -> Result<Snapshot> {
    let mut files = Snapshot::new();
    for entry in walkdir::WalkDir::new(root).follow_links(true) {
        let entry = entry.context("Cannot scan TYPST_FONT_PATHS")?;
        if entry.file_type().is_file()
            && entry.path().extension().is_some_and(|ext| {
                ["ttf", "otf", "ttc", "otc"]
                    .iter()
                    .any(|font| ext.eq_ignore_ascii_case(font))
            })
        {
            files.insert(
                entry.path().strip_prefix(root)?.to_owned(),
                digest(entry.path())?,
            );
        }
    }
    Ok(files)
}

fn absolute_snapshot(snapshots: &BTreeMap<PathBuf, Snapshot>) -> Result<Snapshot> {
    let mut result = Snapshot::new();
    for (root, files) in snapshots {
        for (relative, hash) in files {
            result.insert(fs::canonicalize(root.join(relative))?, hash.clone());
        }
    }
    Ok(result)
}

fn current(snapshot: &Snapshot) -> bool {
    snapshot
        .iter()
        .all(|(path, hash)| digest(path).is_ok_and(|actual| actual == *hash))
}

fn reject_link(path: &Path) -> Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        ensure!(
            !metadata.file_type().is_symlink(),
            "PDF artifact must not be a symbolic link: {}",
            path.display()
        );
    }
    Ok(())
}

fn source_reference(reference: &str) -> Result<String> {
    Ok(format!(
        "/{}",
        source_path(reference)?
            .with_extension("")
            .to_string_lossy()
            .replace('\\', "/")
    ))
}

fn source_path(reference: &str) -> Result<PathBuf> {
    let relative = reference.trim_start_matches('/');
    let mut parts = Vec::new();
    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                ensure!(
                    parts.pop().is_some(),
                    "PDF source escapes input root: {reference}"
                );
            }
            part => parts.push(part),
        }
    }
    ensure!(
        !parts.is_empty() && !reference.contains(['\\', ':', '\0']),
        "Invalid PDF source path: {reference}"
    );
    let path = PathBuf::from(parts.join("/"));
    validate_relative(&path)?;
    Ok(path)
}

fn json_value(value: &impl Serialize) -> Result<String> {
    Ok(format!(
        "json(bytes({}))",
        typst_string(&serde_json::to_string(value)?)
    ))
}

fn typst_string(value: &str) -> String {
    let mut escaped = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_control() => escaped.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => escaped.push(c),
        }
    }
    escaped.push('"');
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_paths_are_portable_and_stay_inside_the_input_root() {
        for (reference, expected) in [
            ("index.typ", "/index"),
            ("/index.html", "/index"),
            ("./a/../b.typ", "/b"),
            ("a/b", "/a/b"),
            ("a/b.v1.typ", "/a/b.v1"),
            (".hidden.typ", "/.hidden"),
        ] {
            assert_eq!(source_reference(reference).unwrap(), expected);
        }
        for reference in [
            "../outside.typ",
            "/a/../../outside.typ",
            "C:/outside.typ",
            "a\\b.typ",
            "",
            "/",
        ] {
            assert!(source_reference(reference).is_err(), "{reference}");
        }
        assert_eq!(
            source_path("./lib/../lib/pdf.typ").unwrap(),
            Path::new("lib/pdf.typ")
        );
    }

    #[test]
    fn typst_literals_escape_code_and_control_characters_without_losing_unicode() {
        assert_eq!(
            typst_string("a\"\\\n\r\t\u{1f}λ"),
            "\"a\\\"\\\\\\n\\r\\t\\u{1f}λ\""
        );
    }
}
