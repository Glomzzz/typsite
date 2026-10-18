//! Atomic replacement shared by exported artifacts, manifests, and site files.

use anyhow::{Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

pub struct PendingFile {
    path: PathBuf,
    file: Option<File>,
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
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error).context("Cannot create temporary artifact"),
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.file
            .as_mut()
            .context("Temporary artifact already closed")?
            .write_all(bytes)
            .with_context(|| format!("Cannot write artifact {}", self.path.display()))
    }

    pub fn commit(self, target: &Path) -> Result<()> {
        self.commit_file(target)?;
        sync_directory(
            target
                .parent()
                .context("Artifact has no parent directory")?,
        )
    }

    /// Disposable cache artifacts need atomic replacement, not durability: a
    /// missing/corrupt file after power loss is verified and rebuilt as a miss.
    pub(crate) fn commit_cache(mut self, target: &Path) -> Result<()> {
        drop(self.file.take());
        fs::rename(&self.path, target)
            .with_context(|| format!("Cannot atomically replace {}", target.display()))
    }

    /// Sync data and atomically rename one published file. A batch caller must
    /// sync each affected parent before advancing its durable checkpoint.
    pub(crate) fn commit_file(mut self, target: &Path) -> Result<()> {
        self.file
            .as_ref()
            .context("Temporary artifact already closed")?
            .sync_data()?;
        drop(self.file.take());
        fs::rename(&self.path, target)
            .with_context(|| format!("Cannot atomically replace {}", target.display()))?;
        Ok(())
    }
}

impl Drop for PendingFile {
    fn drop(&mut self) {
        // Windows cannot remove our temporary file until this handle closes.
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
}

pub(crate) fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

pub(crate) fn write_cache(target: &Path, bytes: &[u8]) -> Result<()> {
    let mut pending = PendingFile::new(target)?;
    pending.write(bytes)?;
    pending.commit_cache(target)
}

pub(crate) fn write_file(target: &Path, bytes: &[u8]) -> Result<()> {
    let mut pending = PendingFile::new(target)?;
    pending.write(bytes)?;
    pending.commit_file(target)
}

pub fn write(target: &Path, bytes: &[u8]) -> Result<()> {
    let mut pending = PendingFile::new(target)?;
    pending.write(bytes)?;
    pending.commit(target)
}
