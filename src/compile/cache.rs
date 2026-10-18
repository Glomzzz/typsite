//! Disposable, versioned cache metadata. Only exported source HTML is cached;
//! resolved relationships and rendered fragments never survive across builds.

use super::project::{Snapshot, digest};
use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const VERSION: u32 = 4;
pub const MANIFEST: &str = "manifest.bin";
const MAGIC: &[u8] = b"TYPSITE-CACHE-4\0";

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
    pub dependencies: Arc<Snapshot>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Publication {
    pub key: String,
    #[serde(default)]
    pub inputs: String,
    pub sources: BTreeSet<PathBuf>,
    pub outputs: Snapshot,
}

// Intern dependency sets on disk and on load. Most independent articles import
// the same library/package files; storing them once avoids quadratic metadata.
#[derive(Serialize, Deserialize)]
struct DiskManifest {
    version: u32,
    export_key: String,
    dependencies: Vec<Arc<Snapshot>>,
    exports: BTreeMap<PathBuf, DiskExport>,
    published: Option<Publication>,
}

#[derive(Serialize, Deserialize)]
struct DiskExport {
    source_hash: String,
    html_hash: String,
    dependencies: usize,
}

impl Manifest {
    pub fn load(root: &Path) -> Self {
        // A missing, old, or corrupt cache is a cache miss, not missing content.
        Self::read(root)
            .filter(|manifest| manifest.version == VERSION)
            .unwrap_or_else(|| Self {
                version: VERSION,
                ..Self::default()
            })
    }

    fn read(root: &Path) -> Option<Self> {
        let bytes = std::fs::read(root.join(MANIFEST)).ok()?;
        let rest = bytes.strip_prefix(MAGIC)?;
        let (checksum, payload) = rest.split_at_checked(32)?;
        if blake3::hash(payload).as_bytes() != checksum {
            return None;
        }
        let disk: DiskManifest = bincode::deserialize(payload).ok()?;
        let mut exports = BTreeMap::new();
        for (path, entry) in disk.exports {
            exports.insert(
                path,
                Export {
                    source_hash: entry.source_hash,
                    html_hash: entry.html_hash,
                    dependencies: disk.dependencies.get(entry.dependencies)?.clone(),
                },
            );
        }
        Some(Self {
            version: disk.version,
            export_key: disk.export_key,
            exports,
            published: disk.published,
        })
    }

    pub fn owns_directory(root: &Path) -> bool {
        Self::read(root).is_some_and(|manifest| manifest.version != 0)
            || std::fs::read(root.join("manifest.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .is_some_and(|manifest| {
                    manifest["version"]
                        .as_u64()
                        .is_some_and(|version| version != 0)
                })
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        let mut sets = BTreeMap::new();
        let mut dependencies = Vec::new();
        let exports: BTreeMap<_, _> = self
            .exports
            .iter()
            .map(|(path, entry)| {
                let index = *sets.entry(entry.dependencies.as_ref()).or_insert_with(|| {
                    dependencies.push(entry.dependencies.clone());
                    dependencies.len() - 1
                });
                (
                    path.clone(),
                    DiskExport {
                        source_hash: entry.source_hash.clone(),
                        html_hash: entry.html_hash.clone(),
                        dependencies: index,
                    },
                )
            })
            .collect();
        let payload = bincode::serialize(&DiskManifest {
            version: self.version,
            export_key: self.export_key.clone(),
            dependencies,
            exports,
            published: self.published.as_ref().map(|published| Publication {
                key: published.key.clone(),
                inputs: published.inputs.clone(),
                sources: published.sources.clone(),
                outputs: published.outputs.clone(),
            }),
        })?;
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(blake3::hash(&payload).as_bytes());
        bytes.extend_from_slice(&payload);
        crate::util::atomic::write_cache(&root.join(MANIFEST), &bytes)
            .context("Cannot save build cache manifest")
    }
}

impl Export {
    /// Reuse one build-local dependency snapshot across articles sharing imports.
    pub fn matches_snapshot(
        &self,
        source_hash: &str,
        html: &Path,
        dependencies: &Snapshot,
    ) -> bool {
        self.source_hash == source_hash
            && digest(html).is_ok_and(|hash| hash == self.html_hash)
            && self
                .dependencies
                .iter()
                .all(|(path, hash)| dependencies.get(path) == Some(hash))
    }
}

/// Check raw exports using one hash per actual dependency, including package
/// files outside the project. This is safe for the early verified no-op path.
pub fn exports_current(
    exports: &BTreeMap<PathBuf, Export>,
    input: &Snapshot,
    root: &Path,
    source_root: &Path,
) -> Result<bool> {
    let mut seen = HashSet::new();
    let mut dependencies = BTreeMap::new();
    for entry in exports.values() {
        if !seen.insert(Arc::as_ptr(&entry.dependencies)) {
            continue;
        }
        for (path, hash) in entry.dependencies.iter() {
            if dependencies
                .insert(path, hash)
                .is_some_and(|previous| previous != hash)
            {
                return Ok(false);
            }
        }
    }
    let current = |(path, expected): &(&PathBuf, &String)| {
        if let Some(hash) = path
            .strip_prefix(source_root)
            .ok()
            .and_then(|relative| input.get(relative))
        {
            hash == *expected
        } else {
            digest(path).is_ok_and(|hash| &hash == *expected)
        }
    };
    let dependencies: Vec<_> = dependencies.into_iter().collect();
    if !(if dependencies.len() < 512 {
        dependencies.iter().all(current)
    } else {
        dependencies.par_iter().all(current)
    }) {
        return Ok(false);
    }
    let targets: Vec<_> = exports
        .keys()
        .map(|relative| relative.with_extension("html"))
        .collect();
    let paths = super::project::managed_paths(root, targets.iter())?;
    let candidates: Vec<_> = exports
        .iter()
        .zip(paths)
        .map(|((relative, entry), path)| (input.get(relative), entry, path))
        .collect();
    let current = |(source_hash, entry, path): &(Option<&String>, &Export, PathBuf)| {
        source_hash.is_some_and(|hash| hash == &entry.source_hash)
            && digest(path).is_ok_and(|hash| hash == entry.html_hash)
    };
    Ok(if candidates.len() < 512 {
        candidates.iter().all(current)
    } else {
        candidates.par_iter().all(current)
    })
}

pub fn fingerprint(value: &impl Serialize) -> Result<String> {
    Ok(blake3::hash(&bincode::serialize(value)?)
        .to_hex()
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_interns_dependencies_and_corruption_or_bad_indices_are_cache_misses() {
        let root = std::env::temp_dir().join(format!(
            "typsite-manifest-intern-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut manifest = Manifest {
            version: VERSION,
            ..Manifest::default()
        };
        let deps = Snapshot::from([(
            PathBuf::from("/shared/import.typ"),
            "dependency-hash".into(),
        )]);
        for name in ["a.typ", "b.typ"] {
            manifest.exports.insert(
                name.into(),
                Export {
                    source_hash: "source".into(),
                    html_hash: "html".into(),
                    dependencies: Arc::new(deps.clone()),
                },
            );
        }
        manifest.save(&root).unwrap();
        let loaded = Manifest::load(&root);
        assert_eq!(loaded.exports.len(), 2);
        assert!(Arc::ptr_eq(
            &loaded.exports[&PathBuf::from("a.typ")].dependencies,
            &loaded.exports[&PathBuf::from("b.typ")].dependencies
        ));
        let mut bytes = std::fs::read(root.join(MANIFEST)).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        std::fs::write(root.join(MANIFEST), &bytes).unwrap();
        assert!(Manifest::load(&root).exports.is_empty());
        manifest.save(&root).unwrap();
        let bytes = std::fs::read(root.join(MANIFEST)).unwrap();
        let mut disk: DiskManifest = bincode::deserialize(&bytes[MAGIC.len() + 32..]).unwrap();
        disk.exports.values_mut().next().unwrap().dependencies = usize::MAX;
        let payload = bincode::serialize(&disk).unwrap();
        let mut bad = MAGIC.to_vec();
        bad.extend_from_slice(blake3::hash(&payload).as_bytes());
        bad.extend_from_slice(&payload);
        std::fs::write(root.join(MANIFEST), bad).unwrap();
        assert!(Manifest::load(&root).exports.is_empty());
        std::fs::remove_file(root.join(MANIFEST)).unwrap();
        std::fs::write(root.join("manifest.json"), r#"{"version":2}"#).unwrap();
        assert!(
            Manifest::owns_directory(&root),
            "legacy cache directories remain cleanable"
        );
        assert!(Manifest::load(&root).exports.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
