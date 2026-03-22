use crate::util::error::TypsiteError;
use anyhow::{anyhow, Context};
use std::fs;
use std::path::Path;

pub fn create_all_parent_dir<P: AsRef<Path>>(path: P) -> anyhow::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Create directory failed while creating file: {path:?}"))?;
    }
    Ok(())
}

pub fn write_into_file<P: AsRef<Path>>(path: P, content: &str, source: &str) -> anyhow::Result<()> {
    create_all_parent_dir(&path)?;
    fs::write(&path, content)
        .map_err(TypsiteError::Io)
        .with_context(|| format!("Failed to write {source} {:?}", path.as_ref()))
}

pub fn remove_file<P: AsRef<Path>>(path: P, source: &str) -> anyhow::Result<()> {
    std::fs::remove_file(path.as_ref())
        .with_context(|| format!("Failed to remove {source}: {:?}", path.as_ref()))
}
pub fn remove_file_log_err<P: AsRef<Path>>(path: P, source: &str) {
    remove_file(path, source).unwrap_or_else(|err| eprintln!("[WARN] {err}"));
}
pub fn remove_file_ignore<P: AsRef<Path>>(path: P) {
    std::fs::remove_file(path.as_ref()).unwrap_or(());
}

pub fn remove_dir_all<P: AsRef<Path>>(path: P) -> anyhow::Result<()> {
    let path_ref = path.as_ref();

    std::fs::remove_dir_all(path_ref)
        .with_context(|| format!("Unable to delete directory: {}", path_ref.display()))?;

    Ok(())
}

pub fn copy_file<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> anyhow::Result<()> {
    create_all_parent_dir(to.as_ref())?;
    std::fs::copy(from.as_ref(), to.as_ref())
        .map_err(|err| {
            anyhow!(
                "Failed to copy {:?} to {:?}: {err}",
                from.as_ref(),
                to.as_ref()
            )
        })
        .map(|_| ())
}
/// Recursively copies a directory and all its contents
pub fn copy_dir<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> anyhow::Result<()> {
    let from_path = from.as_ref();
    let to_path = to.as_ref();

    // Verify source is a directory
    if !from_path.is_dir() {
        return Err(anyhow!(
            "Source path is not a directory: {}",
            from_path.display()
        ));
    }

    // Create the destination directory
    fs::create_dir_all(to_path)
        .with_context(|| format!("Failed to create directory: {}", to_path.display()))?;

    // Start recursive copy process
    copy_dir_recursive(from_path, to_path)?;

    Ok(())
}

/// Internal recursive function for directory copying
fn copy_dir_recursive(from: &Path, to: &Path) -> anyhow::Result<()> {
    // Read source directory entries
    for entry in fs::read_dir(from)
        .with_context(|| format!("Failed to read directory: {}", from.display()))?
    {
        let entry = entry
            .with_context(|| format!("Failed to access directory entry: {}", from.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("Failed to get file type: {}", entry.path().display()))?;

        let target_path = to.join(entry.file_name());

        if file_type.is_dir() {
            // Handle directory: create and recurse
            fs::create_dir_all(&target_path).with_context(|| {
                format!("Failed to create directory: {}", target_path.display())
            })?;
            copy_dir_recursive(&entry.path(), &target_path)?;
        } else if file_type.is_file() {
            // Handle file: copy contents
            copy_file(entry.path(), &target_path).with_context(|| {
                format!(
                    "Failed to copy file from {} to {}",
                    entry.path().display(),
                    target_path.display()
                )
            })?;
        } else if file_type.is_symlink() {
            // Handle symbolic link: recreate link
            let link_target = fs::read_link(entry.path()).with_context(|| {
                format!("Failed to read symbolic link: {}", entry.path().display())
            })?;

            // Platform-specific symlink recreation
            #[cfg(unix)]
            std::os::unix::fs::symlink(&link_target, &target_path).with_context(|| {
                format!(
                    "Failed to create symbolic link: {} -> {}",
                    target_path.display(),
                    link_target.display()
                )
            })?;

            #[cfg(windows)]
            {
                // On Windows, determine if symlink points to file or directory
                let target_metadata = link_target.metadata().with_context(|| {
                    format!("Failed to read target metadata: {}", link_target.display())
                });

                match target_metadata {
                    Ok(meta) if meta.is_dir() => {
                        std::os::windows::fs::symlink_dir(&link_target, &target_path)
                    }
                    _ => std::os::windows::fs::symlink_file(&link_target, &target_path),
                }
                .with_context(|| {
                    format!(
                        "Failed to create symbolic link: {} -> {}",
                        target_path.display(),
                        link_target.display()
                    )
                })?;
            }
        }
        // Other file types (FIFOs, sockets, etc.) are skipped
    }

    Ok(())
}
#[macro_export]
macro_rules! walk_glob {
    ($($arg:tt)*) => {
        {
            let pattern = format!($($arg)*);
            let paths = glob(&pattern)
                .expect(&format!("glob patterns should be valid internal invariants: {pattern}"));
            Box::new(paths.filter_map(std::result::Result::ok))
                as Box<dyn Iterator<Item = std::path::PathBuf> + Send>
        }
    }
}
