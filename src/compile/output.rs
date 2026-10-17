//! A complete output plan with durable ownership and atomic per-file writes.

use super::project::{Snapshot, digest, managed_path, validate_relative};
use super::report::BuildReport;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const OWNERSHIP: &str = ".typsite-outputs.json";
pub const LOCK: &str = ".typsite.lock";

#[derive(Default)]
pub struct OutputPlan {
    files: BTreeMap<PathBuf, (Vec<u8>, String)>,
    names: BTreeMap<String, PathBuf>,
}

#[derive(Default, Serialize, Deserialize)]
struct Ownership {
    files: BTreeSet<PathBuf>,
}

impl Ownership {
    fn load(root: &Path) -> Result<Self> {
        let path = managed_path(root, Path::new(OWNERSHIP))?;
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).context(
                "Invalid output ownership manifest; refusing to guess which files can be removed",
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error).context("Cannot read output ownership manifest"),
        }
    }

    fn save(&self, root: &Path) -> Result<()> {
        crate::util::atomic::write(
            &managed_path(root, Path::new(OWNERSHIP))?,
            &serde_json::to_vec_pretty(self)?,
        )
    }
}

impl OutputPlan {
    pub fn insert(
        &mut self,
        path: PathBuf,
        content: Vec<u8>,
        origin: impl Into<String>,
    ) -> Result<()> {
        validate_relative(&path)?;
        let origin = origin.into();
        ensure!(
            path != Path::new(OWNERSHIP) && path != Path::new(LOCK),
            "Reserved output path: {}",
            path.display()
        );
        let name = path.to_string_lossy().replace('\\', "/").to_lowercase();
        if let Some(previous) = self.names.get(&name) {
            let previous_origin = &self.files[previous].1;
            anyhow::bail!(
                "Output collision for {}: {previous_origin} and {origin}",
                path.display()
            );
        }
        // Detect file/directory collisions too, on every platform, not just at IO.
        for previous in self.names.keys() {
            ensure!(
                !name.starts_with(&format!("{previous}/"))
                    && !previous.starts_with(&format!("{name}/")),
                "Output file/directory collision: {name} and {previous}"
            );
        }
        self.names.insert(name, path.clone());
        self.files.insert(path, (content, origin));
        Ok(())
    }

    pub fn hashes(&self) -> Snapshot {
        self.files
            .iter()
            .map(|(path, (bytes, _))| (path.clone(), blake3::hash(bytes).to_hex().to_string()))
            .collect()
    }

    pub fn is_current(root: &Path, expected: &Snapshot) -> Result<bool> {
        let ownership = Ownership::load(root)?;
        if ownership.files != expected.keys().cloned().collect() {
            return Ok(false);
        }
        for (relative, expected_hash) in expected {
            let path = managed_path(root, relative)?;
            if !digest(&path).is_ok_and(|hash| &hash == expected_hash) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn publish(self, root: &Path, report: &mut BuildReport) -> Result<Snapshot> {
        fs::create_dir_all(root).context("Cannot create output directory")?;
        let hashes = self.hashes();
        let old = Ownership::load(root)?;
        // Validate the entire plan and old manifest before the first write/delete.
        for path in self.files.keys().chain(old.files.iter()) {
            ensure!(
                path != Path::new(OWNERSHIP) && path != Path::new(LOCK),
                "Invalid reserved path in output manifest"
            );
            managed_path(root, path)?;
        }
        let desired: BTreeSet<_> = self.files.keys().cloned().collect();
        // Journal the union first. A failed publication cannot orphan new files;
        // a later build can safely clean them even if the cache was discarded.
        let journal = Ownership {
            files: old.files.union(&desired).cloned().collect(),
        };
        journal.save(root)?;
        for (relative, (bytes, _)) in self.files {
            let path = managed_path(root, &relative)?;
            if fs::read(&path).is_ok_and(|existing| existing == bytes) {
                continue;
            }
            crate::util::atomic::write(&path, &bytes)
                .with_context(|| format!("Cannot publish {}", path.display()))?;
            report.written.push(relative);
        }
        for relative in old.files.difference(&desired) {
            let path = managed_path(root, relative)?;
            match fs::remove_file(&path) {
                Ok(()) => {
                    report.removed.push(relative.clone());
                    prune_empty_parents(root, &path)?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("Cannot remove stale output {}", path.display()));
                }
            }
        }
        Ownership { files: desired }.save(root)?;
        Ok(hashes)
    }
}

fn prune_empty_parents(root: &Path, file: &Path) -> Result<()> {
    let mut parent = file.parent();
    while let Some(path) = parent.filter(|path| *path != root && path.starts_with(root)) {
        if fs::read_dir(path)?.next().is_some() {
            break;
        }
        fs::remove_dir(path)?;
        parent = path.parent();
    }
    Ok(())
}
