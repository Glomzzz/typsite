//! Atomic replacement shared by exported artifacts, manifests, and site files.

use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

pub struct PendingFile {
    path: PathBuf,
}

impl PendingFile {
    pub fn new(target: &Path) -> Result<Self> {
        super::fs::create_all_parent_dir(target)?;
        let parent = target
            .parent()
            .context("Artifact has no parent directory")?;
        let name = target
            .file_name()
            .context("Artifact has no file name")?
            .to_string_lossy();
        loop {
            let path = parent.join(format!(
                ".{name}.{}-{}.tmp",
                std::process::id(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error).context("Cannot create temporary artifact"),
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn commit(self, target: &Path) -> Result<()> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.path)?
            .sync_all()?;
        fs::rename(&self.path, target)
            .with_context(|| format!("Cannot atomically replace {}", target.display()))?;
        #[cfg(unix)]
        fs::File::open(
            target
                .parent()
                .context("Artifact has no parent directory")?,
        )?
        .sync_all()?;
        Ok(())
    }
}

impl Drop for PendingFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn write(target: &Path, bytes: &[u8]) -> Result<()> {
    let pending = PendingFile::new(target)?;
    fs::write(pending.path(), bytes)
        .with_context(|| format!("Cannot write artifact {}", target.display()))?;
    pending.commit(target)
}
