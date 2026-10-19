//! Validated project boundaries and deterministic filesystem discovery.

use anyhow::{Context, Result, bail, ensure};
use rayon::prelude::*;
use std::collections::{BTreeMap, HashSet};
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
            ensure!(
                resolve(packages)? == *packages,
                "Packages path changed through a symbolic link: {}",
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

/// Normalize a portable Typst source-root-relative reference without allowing
/// filesystem prefixes or traversal beyond the source root.
pub(super) fn source_path(reference: &str) -> Result<PathBuf> {
    let mut parts = Vec::new();
    for part in reference.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => ensure!(
                parts.pop().is_some(),
                "Typst source escapes input root: {reference}"
            ),
            part => parts.push(part),
        }
    }
    ensure!(
        !parts.is_empty() && !reference.contains(['\\', ':', '\0']),
        "Invalid Typst source path: {reference}"
    );
    let path = PathBuf::from(parts.join("/"));
    validate_relative(&path)?;
    Ok(path)
}

/// Resolve existing ancestors too, so a symlink cannot disguise overlapping roots.
pub fn resolve(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut resolved = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Normal(name) => {
                resolved.push(name);
                match fs::symlink_metadata(&resolved) {
                    Ok(_) => {
                        resolved = fs::canonicalize(&resolved)
                            .with_context(|| format!("Cannot resolve {}", resolved.display()))?
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(error)
                            .with_context(|| format!("Cannot inspect {}", resolved.display()));
                    }
                }
            }
            root => resolved.push(root.as_os_str()),
        }
    }
    Ok(resolved)
}

pub fn digest(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("Cannot read {}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    if bytes.len() >= 16 * 1024 * 1024 {
        // Large fonts/assets and the engine executable benefit from BLAKE3's
        // tree parallelism; small files avoid task-scheduling overhead.
        hasher.update_rayon(&bytes);
    } else {
        hasher.update(&bytes);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// File symlinks are allowed only inside their source root. Directory symlinks
/// are rejected rather than silently recursed into (or treated as deletions).
pub fn scan(root: &Path) -> Result<Snapshot> {
    fn visit(root: &Path, at: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
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
                files.push(path.strip_prefix(root)?.to_owned());
            } else if kind.is_symlink() {
                let target = fs::canonicalize(&path)
                    .with_context(|| format!("Broken source symlink {}", path.display()))?;
                ensure!(
                    target.starts_with(root) && target.is_file(),
                    "Source symlink must name a file inside {}: {}",
                    root.display(),
                    path.display()
                );
                files.push(path.strip_prefix(root)?.to_owned());
            } else {
                bail!("Unsupported source file type: {}", path.display());
            }
        }
        Ok(())
    }
    // Discover and validate deterministically before hashing independent files.
    // Ordered result folding makes the first error independent of thread timing.
    let mut files = Vec::new();
    visit(root, root, &mut files)?;
    let hash =
        |relative: &PathBuf| digest(&root.join(relative)).map(|hash| (relative.clone(), hash));
    let hashed: Vec<_> = if files.len() < 512 {
        // Tiny projects/no-op CLI invocations should not initialize a full Rayon
        // pool merely to read a few kilobytes of configuration and source.
        files.iter().map(hash).collect()
    } else {
        files.par_iter().map(hash).collect()
    };
    hashed.into_iter().collect()
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

/// Validate a sorted set of managed paths without repeatedly stat-ing shared
/// directories. Files and each distinct ancestor are still inspected for links;
/// missing/file-directory transition paths retain the ordinary managed policy.
pub fn managed_paths<'a>(
    root: &Path,
    relatives: impl IntoIterator<Item = &'a PathBuf>,
) -> Result<Vec<PathBuf>> {
    let mut checked = HashSet::new();
    checked.insert(root.to_owned());
    if let Ok(metadata) = fs::symlink_metadata(root) {
        ensure!(
            !metadata.file_type().is_symlink(),
            "Managed root is a symlink: {}",
            root.display()
        );
    }
    let mut paths = Vec::new();
    for relative in relatives {
        validate_relative(relative)?;
        let mut path = root.to_owned();
        for component in relative.components() {
            path.push(component.as_os_str());
            if checked.insert(path.clone()) {
                match fs::symlink_metadata(&path) {
                    Ok(metadata) => ensure!(
                        !metadata.file_type().is_symlink(),
                        "Managed artifact is a symlink: {}",
                        path.display()
                    ),
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                        ) => {}
                    Err(error) => {
                        return Err(error).with_context(|| {
                            format!("Cannot inspect managed artifact {}", path.display())
                        });
                    }
                }
            }
        }
        paths.push(path);
    }
    Ok(paths)
}

/// Never traverse output/cache symlinks, even for a path loaded from a manifest.
pub fn managed_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    validate_relative(relative)?;
    if let Ok(metadata) = fs::symlink_metadata(root) {
        ensure!(
            !metadata.file_type().is_symlink(),
            "Managed root is a symlink: {}",
            root.display()
        );
    }
    let mut path = root.to_owned();
    for part in relative.components() {
        path.push(part.as_os_str());
        match fs::symlink_metadata(&path) {
            Ok(metadata) => ensure!(
                !metadata.file_type().is_symlink(),
                "Managed artifact is a symlink: {}",
                path.display()
            ),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Cannot inspect managed artifact {}", path.display())
                });
            }
        }
    }
    Ok(path)
}
