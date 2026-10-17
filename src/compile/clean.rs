//! Explicit cleanup shares build locks and validates all roots before deleting.

use super::cache::Manifest;
use super::compiler::lock;
use super::output::{LOCK, OWNERSHIP};
use super::project::resolve;
use anyhow::{Context, Result, ensure};
use std::fs;
use std::path::Path;

pub fn clean_roots(cache: &Path, output: &Path, protected: &[&Path], force: bool) -> Result<()> {
    let cache = resolve(cache)?;
    let output = resolve(output)?;
    ensure!(
        !cache.starts_with(&output) && !output.starts_with(&cache),
        "Cache and output directories must not overlap"
    );
    let cwd = resolve(&std::env::current_dir()?)?;
    let protected = protected
        .iter()
        .map(|path| resolve(path))
        .collect::<Result<Vec<_>>>()?;
    for (root, is_cache) in [(&cache, true), (&output, false)] {
        ensure!(
            !cwd.starts_with(root),
            "Refusing to clean the current directory or an ancestor: {}",
            root.display()
        );
        ensure!(
            !root.join(".git").exists() && !root.join(".jj").exists(),
            "Refusing to clean a repository root: {}",
            root.display()
        );
        for source in &protected {
            ensure!(
                !root.starts_with(source) && !source.starts_with(root),
                "Cleanup path overlaps a source/configuration root: {}",
                root.display()
            );
        }
        if !root.exists() {
            continue;
        }
        ensure!(
            root.is_dir(),
            "Cleanup path is not a directory: {}",
            root.display()
        );
        let nonempty = fs::read_dir(root)?
            .collect::<std::io::Result<Vec<_>>>()?
            .iter()
            .any(|entry| entry.file_name() != LOCK);
        let marked = if is_cache {
            Manifest::owns_directory(root)
        } else {
            root.join(OWNERSHIP).is_file()
        };
        ensure!(
            force || !nonempty || marked,
            "Refusing to clean an unmarked directory: {}. Use --force only if it is disposable legacy cache/output",
            root.display()
        );
    }
    // Acquire every lock before removing anything, and never unlink lock inodes.
    let mut locks = Vec::new();
    for root in [&cache, &output] {
        if root.exists() {
            locks.push(lock(root)?);
        }
    }
    for root in [&cache, &output] {
        if !root.exists() {
            continue;
        }
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if entry.file_name() == LOCK {
                continue;
            }
            let result = if entry.file_type()?.is_dir() {
                fs::remove_dir_all(entry.path())
            } else {
                fs::remove_file(entry.path())
            };
            result.with_context(|| format!("Cannot clean {}", entry.path().display()))?;
        }
    }
    Ok(())
}
