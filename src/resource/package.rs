use anyhow::{Context, Result, anyhow};
use home::home_dir;
use include_dir::{DirEntry, include_dir};
use rayon::iter::{IntoParallelIterator, ParallelBridge, ParallelIterator};
use serde::Deserialize;
use std::{
    fmt,
    fs::{self, create_dir_all},
    path::{Path, PathBuf},
};

use crate::util::fs::{copy_dir, remove_dir_all};

static PACKAGES: include_dir::Dir = include_dir!("./packages/");

pub fn install_packages(packages_path: &Path) -> Result<()> {
    if !packages_path.is_dir() {
        return Err(anyhow!(
            "Path is not a directory: {}",
            packages_path.display()
        ));
    }

    let entries = fs::read_dir(packages_path).with_context(|| {
        format!(
            "Failed to read packages directory: {}",
            packages_path.display()
        )
    })?;

    println!("Installing packages...");
    entries
        .into_iter()
        .par_bridge()
        .filter_map(|entry| {
            entry
                .ok()
                .filter(|it| it.file_type().map(|it| it.is_dir()).unwrap_or(false))
        })
        .map(|entry| -> Result<()> {
            let package_path = entry.path();
            let typst_toml = package_path.join("typst.toml");
            let info = get_package_info(&typst_toml)?;
            install_local(info, |info| install_package(&package_path, info))
        })
        .collect()
}

pub fn install_included_packages() -> Result<()> {
    println!("Installing included packages...");
    PACKAGES
        .entries()
        .into_par_iter()
        .filter_map(|it| it.as_dir())
        .map(|dir| -> Result<()> {
            let info = get_included_package_info(dir)?;
            install_local(info, |info| install_included_package(dir, info))
        })
        .collect::<Result<()>>()
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
            Err(anyhow!("Some packages installed failed!"))
        }
    }
}
fn install_package(package: &Path, info: &PackageInfo) -> Result<()> {
    let local_package_dir = info.get_local_dir()?;
    if local_package_dir.exists() {
        remove_dir_all(&local_package_dir)
            .with_context(|| format!("Failed to clean {local_package_dir:?}"))?;
    }
    copy_dir(package, &local_package_dir)
        .with_context(|| format!("Failed to extract {info} from {local_package_dir:?}"))?;
    Ok(())
}
fn install_included_package(dir: &include_dir::Dir, info: &PackageInfo) -> Result<()> {
    let local_package_dir = info.get_local_dir()?;
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

    pub fn get_local_dir(&self) -> Result<PathBuf> {
        let cache_path = cache_dir().with_context(|| {
            format!("Failed to get cache directory while installing {self} into @local")
        })?;
        let PackageInfo { name, version } = self;
        let local_package =
            cache_path.join(format!("typst/packages/local/{name}/{version}/"));
        Ok(local_package)
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
    Ok(info)
}
