//! Independent raw exports run in a bounded pool. Workers own only their pending
//! artifact; the caller folds results into the manifest after every worker joins.

use super::cache::Export;
use super::project::{ProjectPaths, Snapshot, digest};
use super::typst::{ExportRequest, TypstRunner};
use anyhow::{Context, Result, ensure};
use std::fs;
use std::num::NonZeroUsize;
use std::path::Path;

pub fn export(
    runner: &dyn TypstRunner,
    paths: &ProjectPaths,
    source: &Path,
    source_hash: &str,
    artifact: &Path,
    expected: &Snapshot,
    jobs: NonZeroUsize,
) -> Result<Export> {
    // Borrow the build-wide snapshot for the usual one-pass case. Only exports
    // discovering new/changing dependencies allocate their verification snapshot.
    let mut verified = None;
    for _ in 0..3 {
        let before = verified.as_ref().unwrap_or(expected);
        let pending = crate::util::atomic::PendingFile::new(artifact)?;
        let mut dependencies = runner.export(&ExportRequest {
            root: &paths.input,
            input: source,
            output: pending.path(),
            font_path: &paths.config.join("assets/fonts"),
            jobs,
        })?;
        dependencies.push(source.to_owned());
        let dependencies = dependencies
            .into_iter()
            .map(|path| {
                let path = if path.is_absolute() {
                    path
                } else {
                    paths.input.join(path)
                };
                let path = fs::canonicalize(&path).with_context(|| {
                    format!("Cannot resolve Typst dependency {}", path.display())
                })?;
                Ok((path.clone(), digest(&path)?))
            })
            .collect::<Result<Snapshot>>()?;
        ensure!(
            dependencies
                .get(&fs::canonicalize(source)?)
                .map(String::as_str)
                == Some(source_hash),
            "Source changed during export: {}",
            source.display()
        );
        if dependencies
            .iter()
            .all(|(path, hash)| before.get(path) == Some(hash))
        {
            let html_hash = digest(pending.path())?;
            pending.commit(artifact)?;
            return Ok(Export {
                source_hash: source_hash.to_owned(),
                html_hash,
                dependencies,
            });
        }
        // Newly discovered external inputs need a verification export because
        // their first post-export hash cannot prove what bytes Typst consumed.
        verified = Some(dependencies);
    }
    anyhow::bail!("Typst dependencies did not stabilize during export")
}
