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

pub fn html(body: &str) -> String {
    format!("<head><schema name=\"main\"></schema></head><body>{body}</body>")
}
