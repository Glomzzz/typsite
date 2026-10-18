//! Bounded raw exports with shared before/after dependency snapshots. Workers
//! own pending files; verification and ordered result folding happen after join.

use super::cache::Export;
use super::project::{ProjectPaths, Snapshot, digest};
use super::typst::{ExportRequest, TypstRunner};
use anyhow::{Context, Result, ensure};
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

pub type Job<'a> = (&'a PathBuf, &'a String, PathBuf);

struct Pending {
    file: crate::util::atomic::PendingFile,
    dependencies: BTreeSet<PathBuf>,
    source: PathBuf,
}

pub fn export_batch(
    runner: &dyn TypstRunner,
    paths: &ProjectPaths,
    jobs: &[Job<'_>],
    expected: &Snapshot,
    concurrent: usize,
    typst_jobs: NonZeroUsize,
) -> Result<Vec<Result<Export>>> {
    let pool = if concurrent > 1 {
        Some(
            rayon::ThreadPoolBuilder::new()
                .num_threads(concurrent)
                .build()
                .context("Cannot create Typst export worker pool")?,
        )
    } else {
        None
    };
    let mut results: Vec<_> = std::iter::repeat_with(|| None).take(jobs.len()).collect();
    let mut verified: Vec<Option<Snapshot>> = vec![None; jobs.len()];
    let mut active: Vec<_> = (0..jobs.len()).collect();
    for _ in 0..3 {
        if active.is_empty() {
            break;
        }
        let run = |&index: &usize| -> Result<Pending> {
            let (relative, _, artifact) = &jobs[index];
            let source = paths.input.join(relative);
            let file = crate::util::atomic::PendingFile::new(artifact)?;
            let mut dependencies = runner.export(&ExportRequest {
                root: &paths.input,
                input: &source,
                output: file.path(),
                font_path: &paths.config.join("assets/fonts"),
                jobs: typst_jobs,
            })?;
            dependencies.push(source.clone());
            let canonical = |path: PathBuf| -> Result<PathBuf> {
                let path = if path.is_absolute() {
                    path
                } else {
                    paths.input.join(path)
                };
                if expected.contains_key(&path) {
                    Ok(path)
                } else {
                    fs::canonicalize(&path).with_context(|| {
                        format!("Cannot resolve Typst dependency {}", path.display())
                    })
                }
            };
            let dependencies = dependencies
                .into_iter()
                .map(canonical)
                .collect::<Result<_>>()?;
            Ok(Pending {
                file,
                dependencies,
                source: canonical(source)?,
            })
        };
        // Do not fail fast: successful siblings remain reusable after failures.
        let pending: Vec<_> = if let Some(pool) = &pool {
            pool.install(|| active.par_iter().map(run).collect())
        } else {
            active.iter().map(run).collect()
        };
        // Hash each shared library/package dependency once after the entire
        // batch has finished reading. No artifact is marked current beforehand.
        let dependencies: BTreeSet<_> = pending
            .iter()
            .filter_map(|result| result.as_ref().ok())
            .flat_map(|pending| pending.dependencies.iter())
            .collect();
        let hash = |path: &&PathBuf| ((*path).clone(), digest(path));
        let after: BTreeMap<_, _> = if dependencies.len() < 512 {
            dependencies.iter().map(hash).collect()
        } else {
            dependencies.par_iter().map(hash).collect()
        };
        let mut retry = Vec::new();
        for (index, pending) in active.into_iter().zip(pending) {
            let (_, source_hash, artifact) = &jobs[index];
            let result = (|| -> Result<Option<Export>> {
                let pending = pending?;
                let mut dependencies = Snapshot::new();
                for path in &pending.dependencies {
                    let hash = after[path]
                        .as_ref()
                        .map_err(|error| anyhow::anyhow!("{error:#}"))?;
                    dependencies.insert(path.clone(), hash.clone());
                }
                ensure!(
                    dependencies.get(&pending.source) == Some(*source_hash),
                    "Source changed during export: {}",
                    pending.source.display()
                );
                let before = verified[index].as_ref().unwrap_or(expected);
                if !dependencies
                    .iter()
                    .all(|(path, hash)| before.get(path) == Some(hash))
                {
                    verified[index] = Some(dependencies);
                    return Ok(None);
                }
                let html_hash = digest(pending.file.path())?;
                pending.file.commit_cache(artifact)?;
                dependencies.remove(&pending.source);
                Ok(Some(Export {
                    source_hash: (*source_hash).clone(),
                    html_hash,
                    dependencies: Arc::new(dependencies),
                }))
            })();
            match result {
                Ok(None) => retry.push(index),
                Ok(Some(entry)) => results[index] = Some(Ok(entry)),
                Err(error) => results[index] = Some(Err(error)),
            }
        }
        active = retry;
    }
    Ok(results
        .into_iter()
        .map(|result| {
            result.unwrap_or_else(|| {
                Err(anyhow::anyhow!(
                    "Typst dependencies did not stabilize during export"
                ))
            })
        })
        .collect())
}
