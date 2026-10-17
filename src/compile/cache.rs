//! Disposable, versioned cache metadata. Only exported source HTML is cached;
//! resolved relationships and rendered fragments never survive across builds.

use super::project::{Snapshot, digest};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const VERSION: u32 = 1;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    version: u32,
    pub export_key: String,
    pub exports: BTreeMap<PathBuf, Export>,
    pub published: Option<Publication>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Export {
    pub source_hash: String,
    pub html_hash: String,
    pub dependencies: Snapshot,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Publication {
    pub key: String,
    pub outputs: Snapshot,
}

impl Manifest {
    pub fn load(root: &Path) -> Self {
        // A missing, old, or corrupt cache is a cache miss, not missing content.
        std::fs::read(root.join("manifest.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Self>(&bytes).ok())
            .filter(|manifest| manifest.version == VERSION)
            .unwrap_or_else(|| Self {
                version: VERSION,
                ..Self::default()
            })
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        crate::util::atomic::write(
            &root.join("manifest.json"),
            &serde_json::to_vec_pretty(self)?,
        )
        .context("Cannot save build cache manifest")
    }
}

impl Export {
    pub fn is_current(&self, source_hash: &str, html: &Path) -> bool {
        self.source_hash == source_hash
            && digest(html).is_ok_and(|hash| hash == self.html_hash)
            && self
                .dependencies
                .iter()
                .all(|(path, hash)| digest(path).is_ok_and(|current| &current == hash))
    }
}

pub fn fingerprint(value: &impl Serialize) -> Result<String> {
    Ok(blake3::hash(&serde_json::to_vec(value)?)
        .to_hex()
        .to_string())
}
