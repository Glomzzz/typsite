use anyhow::{Context, Result, anyhow};
use home::home_dir;
use include_dir::{DirEntry, include_dir};
use serde::Deserialize;
use std::collections::HashSet;
use std::{
    fmt,
    fs::{self, create_dir_all},
    path::{Path, PathBuf},
};

use crate::util::fs::{copy_dir, remove_dir_all};

static PACKAGES: include_dir::Dir = include_dir!("./packages/");

pub fn install_packages(packages_path: &Path) -> Result<()> {
    install_packages_into(packages_path, &default_namespace()?)
}

pub fn install_included_packages() -> Result<()> {
    install_included_into(&default_namespace()?)
}

pub fn install_packages_into(packages_path: &Path, namespace: &Path) -> Result<()> {
    let mut entries = fs::read_dir(packages_path)
        .with_context(|| format!("Cannot read packages directory {}", packages_path.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut packages = Vec::new();
    let mut identities = HashSet::new();
    // Validate every identity before mutating the installation namespace.
    for entry in entries {
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let info = get_package_info(&entry.path().join("typst.toml"))?;
        anyhow::ensure!(
            identities.insert((info.name.clone(), info.version.clone())),
            "Duplicate package {info}"
        );
        packages.push((entry.path(), info));
    }
    let _lock = lock_namespace(namespace)?;
    for (path, info) in packages {
        install_local(info, |info| install_package(&path, info, namespace))?;
    }
    Ok(())
}

pub fn install_included_into(namespace: &Path) -> Result<()> {
    let _lock = lock_namespace(namespace)?;
    for dir in PACKAGES.entries().iter().filter_map(|entry| entry.as_dir()) {
        let info = get_included_package_info(dir)?;
        install_local(info, |info| install_included_package(dir, info, namespace))?;
    }
    Ok(())
}

pub fn default_namespace() -> Result<PathBuf> {
    for key in ["TYPST_PACKAGE_PATH", "TYPST_PACKAGE_CACHE_PATH"] {
        if let Some(path) = std::env::var_os(key).filter(|path| !path.is_empty()) {
            return Ok(PathBuf::from(path));
        }
    }
    Ok(cache_dir()?.join("typst/packages"))
}

fn lock_namespace(root: &Path) -> Result<fs::File> {
    fs::create_dir_all(root)?;
    let path = crate::compile::project::managed_path(root, Path::new(".typsite-package.lock"))?;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock()
        .with_context(|| format!("Package namespace is in use: {}", root.display()))?;
    Ok(file)
}

fn install_local(
    info: PackageInfo,
    install: impl FnOnce(&PackageInfo) -> Result<()>,
) -> Result<()> {
    println!(" - Installing {info}");
    match install(&info) {
        Ok(_) => {
            println!("   - Successfully installed {info}");
            Ok(())
        }
        Err(err) => {
            eprintln!("   - Failed to install {info}, because: {err}");
            Err(err).with_context(|| format!("Cannot install {info}"))
        }
    }
}
fn install_package(package: &Path, info: &PackageInfo, namespace: &Path) -> Result<()> {
    let local_package_dir = info.local_dir(namespace)?;
    if local_package_dir.exists() {
        remove_dir_all(&local_package_dir)
            .with_context(|| format!("Failed to clean {local_package_dir:?}"))?;
    }
    copy_dir(package, &local_package_dir)
        .with_context(|| format!("Failed to extract {info} from {local_package_dir:?}"))?;
    Ok(())
}
fn install_included_package(
    dir: &include_dir::Dir,
    info: &PackageInfo,
    namespace: &Path,
) -> Result<()> {
    let local_package_dir = info.local_dir(namespace)?;
    if local_package_dir.exists() {
        remove_dir_all(&local_package_dir)?;
    }
    create_dir_all(&local_package_dir)
        .with_context(|| format!("Failed to create {info} directory in {local_package_dir:?}"))?;
    let dir_name = dir
        .path()
        .to_str()
        .with_context(|| format!("Failed to get dir name of {:?}", dir.path()))?;
    extract(dir, dir_name, &local_package_dir)
}

fn get_included_package_info(dir: &include_dir::Dir) -> Result<PackageInfo> {
    let path = format!("{}/typst.toml", dir.path().to_string_lossy());
    let typst_toml = dir
        .get_file(&path)
        .with_context(|| format!("Failed to get `typst.toml` in included `{path:?}`"))?;
    let content = typst_toml
        .contents_utf8()
        .with_context(|| format!("Failed to read `typst.toml` in included `{:?}`", dir.path()))?;
    get_package_info_from_content(content).with_context(|| {
        format!(
            "Failed to parse `typst.toml` in included `{:?}`",
            dir.path()
        )
    })
}

fn extract<S: AsRef<Path>>(
    dir: &include_dir::Dir,
    root_dir_name: &str,
    base_path: S,
) -> Result<()> {
    let base_path = base_path.as_ref();

    for entry in dir.entries() {
        let entry_path = entry.path().strip_prefix(root_dir_name).with_context(|| {
            format!(
                "Failed to strip_prefix starts form {:?} for {:?}",
                dir.path(),
                entry.path()
            )
        })?;
        let path = base_path.join(entry_path);

        match entry {
            DirEntry::Dir(d) => {
                fs::create_dir_all(&path)
                    .with_context(|| format!("Failed to create dir all for {path:?}"))?;
                extract(d, root_dir_name, base_path)?;
            }
            DirEntry::File(f) => {
                fs::write(&path, f.contents())
                    .with_context(|| format!("Failed to write into {path:?}"))?;
            }
        }
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct PackageInfo {
    name: String,
    version: String,
}

impl fmt::Display for PackageInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@local/{}:{}", self.name, self.version)?;
        Ok(())
    }
}

/**
* Get the cache directory based on the operating system.
*
* - On Linux: `$HOME/.cache`
* - On macOS: `$HOME/Library/Caches`
* - On Windows: `$HOME/AppData/Local`
*/
fn cache_dir() -> Result<PathBuf> {
    let cache_path = home_dir().with_context(|| "Failed to get home directory")?;
    let cache_path = if cfg!(target_os = "linux") {
        cache_path.join(".cache")
    } else if cfg!(target_os = "macos") {
        cache_path.join("Library").join("Caches")
    } else if cfg!(target_os = "windows") {
        cache_path.join("AppData").join("Local")
    } else {
        return Err(anyhow!("Unsupported OS"));
    };
    Ok(cache_path)
}

impl PackageInfo {
    fn local_dir(&self, namespace: &Path) -> Result<PathBuf> {
        crate::compile::project::managed_path(
            namespace,
            &Path::new("local").join(&self.name).join(&self.version),
        )
    }
}

#[derive(Debug, Deserialize)]
struct TypstToml {
    package: PackageInfo,
}

fn get_package_info(typst_toml: &Path) -> Result<PackageInfo> {
    let content = fs::read_to_string(typst_toml)
        .with_context(|| format!("Failed to read `{typst_toml:?}`"))?;
    get_package_info_from_content(&content)
}
fn get_package_info_from_content(toml_str: &str) -> Result<PackageInfo> {
    let info = toml::from_str::<TypstToml>(toml_str)?.package;
    for (field, value) in [("name", &info.name), ("version", &info.version)] {
        let path = Path::new(value);
        crate::compile::project::validate_relative(path)
            .with_context(|| format!("Invalid package {field}: {value}"))?;
        anyhow::ensure!(
            path.components().count() == 1 && !value.starts_with('.'),
            "Invalid package {field}: {value}"
        );
    }
    Ok(info)
}
