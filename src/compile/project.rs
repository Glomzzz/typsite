//! Validated project boundaries and deterministic filesystem discovery.

use anyhow::{Context, Result, bail, ensure};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub type Snapshot = BTreeMap<PathBuf, String>;

#[derive(Debug, Clone)]
pub struct ProjectPaths {
    pub input: PathBuf,
    pub config: PathBuf,
    pub cache: PathBuf,
    pub output: PathBuf,
    pub packages: Option<PathBuf>,
}

impl ProjectPaths {
    pub fn new(
        input: PathBuf,
        config: PathBuf,
        cache: PathBuf,
        output: PathBuf,
        packages: Option<PathBuf>,
    ) -> Result<Self> {
        let paths = Self {
            input: resolve(&input)?,
            config: resolve(&config)?,
            cache: resolve(&cache)?,
            output: resolve(&output)?,
            packages: packages.map(|path| resolve(&path)).transpose()?,
        };
        paths.validate()?;
        Ok(paths)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.input.is_dir(),
            "Input directory does not exist: {}",
            self.input.display()
        );
        ensure!(
            self.config.is_dir(),
            "Configuration directory does not exist: {}",
            self.config.display()
        );
        let roots = [
            ("input", &self.input),
            ("config", &self.config),
            ("cache", &self.cache),
            ("output", &self.output),
        ];
        for (index, (name, path)) in roots.iter().enumerate() {
            ensure!(
                resolve(path)? == **path,
                "Project {name} path changed through a symbolic link: {}",
                path.display()
            );
            for (other_name, other) in &roots[index + 1..] {
                ensure!(
                    !path.starts_with(other) && !other.starts_with(path),
                    "Project {name} and {other_name} directories must not overlap: {} and {}",
                    path.display(),
                    other.display()
                );
            }
        }
        if let Some(packages) = &self.packages {
            ensure!(
                packages.is_dir(),
                "Packages directory does not exist: {}",
                packages.display()
            );
            for writable in [&self.cache, &self.output] {
                ensure!(
                    !packages.starts_with(writable) && !writable.starts_with(packages),
                    "Packages directory overlaps a writable project directory"
                );
            }
        }
        Ok(())
    }
}

/// Resolve existing ancestors too, so a symlink cannot disguise overlapping roots.
pub fn resolve(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    let mut ancestor = normalized.as_path();
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                suffix.push(
                    ancestor
                        .file_name()
                        .context("Cannot resolve project path")?
                        .to_owned(),
                );
                ancestor = ancestor
                    .parent()
                    .context("Cannot resolve project ancestor")?;
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Cannot inspect {}", ancestor.display()));
            }
        }
    }
    let mut resolved = fs::canonicalize(ancestor)
        .with_context(|| format!("Cannot resolve {}", ancestor.display()))?;
    for name in suffix.into_iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}

pub fn digest(path: &Path) -> Result<String> {
    Ok(
        blake3::hash(&fs::read(path).with_context(|| format!("Cannot read {}", path.display()))?)
            .to_hex()
            .to_string(),
    )
}

/// File symlinks are allowed only inside their source root. Directory symlinks
/// are rejected rather than silently recursed into (or treated as deletions).
pub fn scan(root: &Path) -> Result<Snapshot> {
    fn visit(root: &Path, at: &Path, files: &mut Snapshot) -> Result<()> {
        let mut entries = fs::read_dir(at)
            .with_context(|| format!("Cannot scan {}", at.display()))?
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let kind = entry.file_type()?;
            if kind.is_dir() {
                visit(root, &path, files)?;
            } else if kind.is_file() {
                files.insert(path.strip_prefix(root)?.to_owned(), digest(&path)?);
            } else if kind.is_symlink() {
                let target = fs::canonicalize(&path)
                    .with_context(|| format!("Broken source symlink {}", path.display()))?;
                ensure!(
                    target.starts_with(root) && target.is_file(),
                    "Source symlink must name a file inside {}: {}",
                    root.display(),
                    path.display()
                );
                files.insert(path.strip_prefix(root)?.to_owned(), digest(&path)?);
            } else {
                bail!("Unsupported source file type: {}", path.display());
            }
        }
        Ok(())
    }
    let mut files = Snapshot::new();
    visit(root, root, &mut files)?;
    Ok(files)
}

/// Cross-platform output/cache names: no roots, traversal, drive prefixes, or
/// alternate separators that would mean something different on another OS.
pub fn validate_relative(path: &Path) -> Result<()> {
    ensure!(!path.as_os_str().is_empty(), "Artifact path is empty");
    ensure!(
        path.components()
            .all(|part| matches!(part, Component::Normal(_))),
        "Artifact path must stay relative to its managed root: {}",
        path.display()
    );
    let text = path
        .to_str()
        .context("Artifact path is not valid Unicode")?;
    ensure!(
        !text.contains([':', '\0']) && (cfg!(windows) || !text.contains('\\')),
        "Artifact path is not portable: {}",
        path.display()
    );
    Ok(())
}

/// Never traverse output/cache symlinks, even for a path loaded from a manifest.
pub fn managed_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    validate_relative(relative)?;
    let mut path = root.to_owned();
    for part in relative.components() {
        path.push(part.as_os_str());
        match fs::symlink_metadata(&path) {
            Ok(metadata) => ensure!(
                !metadata.file_type().is_symlink(),
                "Managed artifact is a symlink: {}",
                path.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Cannot inspect managed artifact {}", path.display())
                });
            }
        }
    }
    Ok(path)
}
