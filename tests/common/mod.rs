#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use typsite::compile::options::CompileOptions;
use typsite::config::TypsiteConfig;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// A self-cleaning project. Tests never change cwd, HOME, or installed packages.
pub struct Project {
    pub root: PathBuf,
    pub config: PathBuf,
    pub input: PathBuf,
    pub cache: PathBuf,
    pub output: PathBuf,
}

impl Project {
    pub fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "typsite-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let project = Self {
            config: root.join(".typsite"),
            input: root.join("content"),
            cache: root.join("cache"),
            output: root.join("publish"),
            root,
        };
        typsite::util::fs::copy_dir(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/.typsite"),
            &project.config,
        )
        .unwrap();
        fs::create_dir_all(&project.input).unwrap();
        for (path, source) in [
            (
                "schemas/main.html",
                "<head></head><body><content></content><footer></footer></body>",
            ),
            (
                "schemas/reference.html",
                "<head></head><body><a href=\"{slug}\">Reference {slug}</a></body>",
            ),
            (
                "schemas/backlink.html",
                "<head></head><body><a href=\"{slug}\">Backlink {slug}</a></body>",
            ),
            (
                "components/footer.html",
                "<head></head><body><footer>{references}{backlinks}</footer></body>",
            ),
            (
                "components/footer/references.html",
                "<head></head><body><div class=\"references\">{references}</div></body>",
            ),
            (
                "components/footer/backlinks.html",
                "<head></head><body><div class=\"backlinks\">{backlinks}</div></body>",
            ),
        ] {
            project.configure(path, source);
        }
        project
    }

    pub fn configure(&self, path: &str, source: &str) {
        self.write(self.config.join(path), source);
    }

    pub fn source(&self, path: &str, source: &str) {
        self.write(self.input.join(path), source);
    }

    pub fn write(&self, path: PathBuf, source: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }

    pub fn load(&self, options: CompileOptions) -> TypsiteConfig<'_> {
        TypsiteConfig::load_with_options(&self.config, &self.input, &self.cache, options).unwrap()
    }

    pub fn snapshot(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn visit(root: &Path, at: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
            if !at.exists() {
                return;
            }
            for entry in fs::read_dir(at).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    visit(root, &path, files);
                } else {
                    files.insert(
                        path.strip_prefix(root).unwrap().to_owned(),
                        fs::read(path).unwrap(),
                    );
                }
            }
        }
        let mut files = BTreeMap::new();
        visit(&self.output, &self.output, &mut files);
        files
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn successful(
    compiler: &typsite::compile::compiler::Compiler,
) -> typsite::compile::report::BuildReport {
    let report = compiler
        .compile()
        .expect("build infrastructure should succeed");
    assert!(report.is_success(), "{:#?}", report.diagnostics);
    report
}

pub fn clean_equivalent(project: &Project, compiler: &typsite::compile::compiler::Compiler) {
    let incremental = project.snapshot();
    fs::remove_dir_all(&project.cache).unwrap();
    fs::remove_dir_all(&project.output).unwrap();
    successful(compiler);
    assert_eq!(
        incremental,
        project.snapshot(),
        "incremental output must equal a clean build"
    );
}

pub fn html(body: &str) -> String {
    format!("<head><schema name=\"main\"></schema></head><body>{body}</body>")
}

#[derive(Default)]
pub struct FakeTypst {
    calls: std::sync::Mutex<Vec<PathBuf>>,
    dependencies: std::sync::Mutex<BTreeMap<String, Vec<String>>>,
    pub fail_next: std::sync::atomic::AtomicBool,
    pub revision: std::sync::atomic::AtomicUsize,
    pub edit_during_export: std::sync::Mutex<Option<(PathBuf, String)>>,
}

impl FakeTypst {
    pub fn calls(&self) -> Vec<PathBuf> {
        self.calls.lock().unwrap().clone()
    }
    pub fn depends_on(&self, source: &str, dependency: &str) {
        self.dependencies
            .lock()
            .unwrap()
            .entry(source.into())
            .or_default()
            .push(dependency.into());
    }
}

impl typsite::compile::typst::TypstRunner for FakeTypst {
    fn identity(&self) -> anyhow::Result<String> {
        Ok(format!(
            "fake Typst 0.15.1 revision {}",
            self.revision.load(Ordering::SeqCst)
        ))
    }
    fn export(
        &self,
        request: &typsite::compile::typst::ExportRequest<'_>,
    ) -> anyhow::Result<Vec<PathBuf>> {
        assert!(request.root.is_absolute());
        assert!(request.input.starts_with(request.root));
        let relative = request.input.strip_prefix(request.root)?.to_owned();
        self.calls.lock().unwrap().push(relative.clone());
        if self.fail_next.swap(false, Ordering::SeqCst) {
            fs::write(request.output, "partial failed export")?;
            anyhow::bail!("deliberate export failure");
        }
        let mut source = fs::read_to_string(request.input)?;
        let mut dependencies = vec![request.input.to_owned()];
        for dependency in self
            .dependencies
            .lock()
            .unwrap()
            .get(relative.to_string_lossy().replace('\\', "/").as_str())
            .into_iter()
            .flatten()
        {
            let dependency = request.root.join(dependency);
            source = source.replace("{{include}}", &fs::read_to_string(&dependency)?);
            dependencies.push(dependency);
        }
        if let Some((path, content)) = self.edit_during_export.lock().unwrap().take() {
            fs::write(path, content)?;
        }
        fs::write(request.output, source)?;
        Ok(dependencies)
    }
}

impl Project {
    pub fn compiler(
        &self,
        runner: std::sync::Arc<dyn typsite::compile::typst::TypstRunner>,
    ) -> typsite::compile::compiler::Compiler {
        self.compiler_with_options(runner, CompileOptions::default())
    }
    pub fn compiler_with_options(
        &self,
        runner: std::sync::Arc<dyn typsite::compile::typst::TypstRunner>,
        options: CompileOptions,
    ) -> typsite::compile::compiler::Compiler {
        let paths = typsite::compile::project::ProjectPaths::new(
            self.input.clone(),
            self.config.clone(),
            self.cache.clone(),
            self.output.clone(),
            None,
        )
        .unwrap();
        typsite::compile::compiler::Compiler::with_runner(paths, options, runner).unwrap()
    }
}
