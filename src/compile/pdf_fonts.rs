//! Explicit, pinned HTTPS font profiles. No system installation or bundled font
//! binaries: only verified disposable cache files are passed to native Typst.

use super::super::project::{managed_path, scan};
use crate::util::atomic::write_cache;
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum FontProfile {
    #[default]
    None,
    NotoSerifSc,
}

impl FontProfile {
    pub fn family(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::NotoSerifSc => Some("Noto Serif SC"),
        }
    }
}

const REVISION: &str = "9b0f1436e455d902de067a2501422e5dc71ad16b";
const BASE: &str = "https://raw.githubusercontent.com/notofonts/noto-cjk/9b0f1436e455d902de067a2501422e5dc71ad16b/Serif";
const MAX_DOWNLOAD: u64 = 32 * 1024 * 1024;

struct Asset<'a> {
    name: &'a str,
    source: &'a str,
    sha256: &'a str,
}

const NOTO: &[Asset<'static>] = &[
    Asset {
        name: "NotoSerifSC-Regular.otf",
        source: "SubsetOTF/SC/NotoSerifSC-Regular.otf",
        sha256: "e8f396decc1f0963a016a989c3d8852e863d1350996f573860a80767c83a1cd3",
    },
    Asset {
        name: "NotoSerifSC-Medium.otf",
        source: "SubsetOTF/SC/NotoSerifSC-Medium.otf",
        sha256: "e4e9beb5aaeb951e101aa8f122e2a6801ea9e48ac9f9b6bdfeef2436b55bc8ee",
    },
    Asset {
        name: "OFL.txt",
        source: "LICENSE",
        sha256: "6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2",
    },
];

fn checksum(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Caller holds the project cache lock. Only the built-in immutable upstream
/// revision is contacted; custom URL fetching and unverified CSS are not used.
pub(super) fn prepare(cache: &Path, profile: FontProfile, offline: bool) -> Result<Vec<PathBuf>> {
    prepare_with(cache, profile, offline, |url| {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(3)
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_global(Some(Duration::from_secs(120)))
            .build()
            .into();
        let mut response = agent.get(url).call()
            .with_context(|| format!("Cannot download PDF font from {url}; prewarm the cache online before using --offline"))?;
        response
            .body_mut()
            .with_config()
            .limit(MAX_DOWNLOAD)
            .read_to_vec()
            .context("Cannot read PDF font download")
    })
}

fn prepare_with(
    cache: &Path,
    profile: FontProfile,
    offline: bool,
    mut download: impl FnMut(&str) -> Result<Vec<u8>>,
) -> Result<Vec<PathBuf>> {
    if profile == FontProfile::None {
        return Ok(Vec::new());
    }
    let relative = format!("fonts/noto-serif-sc-{REVISION}");
    let directory = managed_path(cache, Path::new(&relative))?;
    prepare_assets(&directory, NOTO, offline, &mut download)?;
    Ok(vec![directory])
}

fn prepare_assets(
    directory: &Path,
    assets: &[Asset<'_>],
    offline: bool,
    mut download: impl FnMut(&str) -> Result<Vec<u8>>,
) -> Result<()> {
    fs::create_dir_all(directory)?;
    // Reject symlinks and unexpected files: another font in this directory must
    // not silently override the selected pinned family during font discovery.
    for path in scan(directory)?.keys() {
        ensure!(
            assets.iter().any(|asset| path == Path::new(asset.name)),
            "Unexpected file in managed PDF font cache: {}",
            path.display()
        );
    }
    for asset in assets {
        let target = managed_path(directory, Path::new(asset.name))?;
        if fs::read(&target).is_ok_and(|bytes| checksum(&bytes) == asset.sha256) {
            continue;
        }
        ensure!(
            !offline,
            "Verified PDF font cache missing or corrupt: {}. Run once without --offline to fetch --font-profile noto-serif-sc",
            target.display()
        );
        eprintln!(
            "Downloading PDF font resource {} (Noto Serif SC 2.003)",
            asset.name
        );
        let bytes = download(&format!("{BASE}/{}", asset.source))?;
        ensure!(
            bytes.len() as u64 <= MAX_DOWNLOAD && checksum(&bytes) == asset.sha256,
            "PDF font SHA-256 mismatch for {}; last successful PDF retained",
            asset.name
        );
        if asset.name.ends_with(".otf") {
            ensure!(
                bytes.starts_with(b"OTTO"),
                "Downloaded PDF font is not an OpenType font"
            );
        }
        write_cache(&target, &bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "typsite-font-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn disabled_and_offline_profiles_do_not_use_the_network() {
        let root = fixture();
        assert!(
            prepare_with(&root, FontProfile::None, false, |_| panic!(
                "Network must be opt-in"
            ))
            .unwrap()
            .is_empty()
        );
        let error = prepare_with(&root, FontProfile::NotoSerifSc, true, |_| {
            panic!("Offline must not connect")
        })
        .unwrap_err();
        assert!(error.to_string().contains("missing or corrupt"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wrong_downloads_are_not_cached_and_download_errors_propagate() {
        let root = fixture();
        let error = prepare_with(&root, FontProfile::NotoSerifSc, false, |_| {
            Ok(b"Not a trusted font".to_vec())
        })
        .unwrap_err();
        assert!(error.to_string().contains("SHA-256 mismatch"));
        assert!(scan(&root).unwrap().is_empty());
        let error = prepare_with(&root, FontProfile::NotoSerifSc, false, |_| {
            anyhow::bail!("deliberate network failure")
        })
        .unwrap_err();
        assert!(error.to_string().contains("deliberate network failure"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verified_downloads_are_reused_offline_and_corrupt_caches_are_repaired() {
        let root = fixture();
        let bytes = b"OTTO test font bytes";
        let hash = checksum(bytes);
        let assets = [Asset {
            name: "font.otf",
            source: "font.otf",
            sha256: &hash,
        }];
        let mut calls = 0;
        prepare_assets(&root, &assets, false, |_| {
            calls += 1;
            Ok(bytes.to_vec())
        })
        .unwrap();
        assert_eq!(calls, 1);
        let modified = fs::metadata(root.join("font.otf"))
            .unwrap()
            .modified()
            .unwrap();
        prepare_assets(&root, &assets, true, |_| {
            panic!("Verified cache must not connect")
        })
        .unwrap();
        assert_eq!(
            fs::metadata(root.join("font.otf"))
                .unwrap()
                .modified()
                .unwrap(),
            modified
        );
        fs::write(root.join("font.otf"), "corrupt").unwrap();
        assert!(
            prepare_assets(&root, &assets, true, |_| panic!("Offline must not connect")).is_err()
        );
        prepare_assets(&root, &assets, false, |_| Ok(bytes.to_vec())).unwrap();
        assert_eq!(fs::read(root.join("font.otf")).unwrap(), bytes);
        fs::write(root.join("untrusted.otf"), bytes).unwrap();
        assert!(
            prepare_assets(&root, &assets, false, |_| panic!(
                "No network for unexpected files"
            ))
            .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sha256_matches_the_standard_known_vector() {
        assert_eq!(
            checksum(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[cfg(unix)]
    #[test]
    fn linked_font_cache_is_rejected_without_reading_or_modifying_the_target() {
        let root = fixture();
        let victim = root.join("victim");
        fs::create_dir_all(&victim).unwrap();
        fs::write(victim.join("protected.txt"), "Keep this").unwrap();
        std::os::unix::fs::symlink(&victim, root.join("fonts")).unwrap();
        assert!(
            prepare_with(&root, FontProfile::NotoSerifSc, false, |_| panic!(
                "No network for unsafe path"
            ))
            .is_err()
        );
        assert_eq!(
            fs::read_to_string(victim.join("protected.txt")).unwrap(),
            "Keep this"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
