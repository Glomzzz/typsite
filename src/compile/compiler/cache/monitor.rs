use crate::compile::compiler::PathBufs;
use crate::util::error::{log_err, log_err_or_ok};
use crate::util::fs::{
    copy_file, remove_file, remove_file_ignore, remove_file_log_err, write_into_file,
};
use crate::util::path::relative_path;
use crate::walk_glob;
use anyhow::{Context, Result};
use blake3::{Hash, Hasher};
use glob::glob;
use memmap2::Mmap;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::str::FromStr;
use std::{fs::File, path::Path};

pub struct Monitor<'a> {
    config_path: &'a Path,
    typst_path: &'a Path,
    html_cache_path: &'a Path,
    hash_path: PathBuf,
    retry_path: PathBuf,
    typst_hash_cache: HashMap<PathBuf, Hash>,
    non_typst_hash_cache: HashMap<PathBuf, Hash>,
    html_hash_cache: HashMap<PathBuf, Hash>,
    config_hash_cache: HashMap<PathBuf, Hash>,
}

impl<'a> Monitor<'a> {
    pub fn load(
        cache_path: &Path,
        config_path: &'a Path,
        typst_path: &'a Path,
        html_cache_path: &'a Path,
    ) -> Monitor<'a> {
        let hash_path = cache_path.join("hash");
        let retry_path = hash_path.join("retry");
        let typst_hash_cache = load_hashes(&hash_path, &hash_path.join(typst_path), ".typ");
        let non_typst_hash_cache = load_hashes(&hash_path, &hash_path.join(typst_path), "[!.typ]");
        let html_hash_cache = load_hashes(&hash_path, &hash_path.join(html_cache_path), "");
        let config_hash_cache = load_hashes(&hash_path, &hash_path.join(config_path), "");
        Self {
            config_path,
            typst_path,
            hash_path,
            html_cache_path,
            retry_path,
            typst_hash_cache,
            non_typst_hash_cache,
            html_hash_cache,
            config_hash_cache,
        }
    }
    pub fn refresh_html(
        &mut self,
        deleted_typst_paths: &PathBufs,
        overall_compile_needed: bool,
    ) -> Result<Vec<PathBuf>> {
        deleted_typst_paths
            .par_iter()
            .map(|it| self.html_cache_path.join(it).with_extension("html"))
            .for_each(|it| remove_file_log_err(it, "cache html"));
        let pattern = format!("{}/**/*.html", self.html_cache_path.display());
        let hash_new: HashMap<PathBuf, Hash> = hash_pattern(&pattern).into_iter().collect();
        let all_htmls: Vec<PathBuf> = hash_new.keys().cloned().collect();
        let (updated, _) = refresh(
            &self.hash_path,
            Some(&self.retry_path),
            &mut self.html_hash_cache,
            hash_new,
        )?;
        Ok(if overall_compile_needed {
            all_htmls
        } else {
            updated.into_iter().collect()
        })
    }

    pub fn refresh_config(&mut self) -> Result<(PathBufs, PathBufs)> {
        let pattern = format!("{}/**/*", self.config_path.display());
        let hash_new = hash_pattern(&pattern).into_iter().collect();
        refresh(&self.hash_path, None, &mut self.config_hash_cache, hash_new)
    }

    pub fn refresh_typst(&mut self) -> Result<(PathBufs, PathBufs, PathBufs)> {
        let pattern = format!("{}/**/*.typ", self.typst_path.display());
        let hash_new: HashMap<PathBuf, Hash> = hash_pattern(&pattern).into_iter().collect();
        let all_typsts: PathBufs = hash_new.keys().cloned().collect();
        refresh(&self.hash_path, None, &mut self.typst_hash_cache, hash_new)
            .map(|(updated, deleted)| (all_typsts, updated, deleted))
    }

    pub fn refresh_non_typst(&mut self) -> Result<(PathBufs, PathBufs)> {
        let pattern = format!("{}/**/*[!.typ]", self.typst_path.display());
        let hash_new = hash_pattern(&pattern).into_iter().collect();
        refresh(
            &self.hash_path,
            None,
            &mut self.non_typst_hash_cache,
            hash_new,
        )
    }

    // Remember those (failed) html files, an attempt will be made to load them next time
    pub fn retry_next_time(&self, cache_html_path: &Path) {
        let html_hash = cache_html_path.with_extension("html.hash");
        let html_hash_path = self.hash_path.join(&html_hash);
        let retry_path = self.retry_path.join(&html_hash);
        copy_file(html_hash_path, retry_path).unwrap_or_else(|err| eprintln!("{err}"));
    }

    pub fn remove_retry_hash(&self, path: &Path) {
        let path = self
            .retry_path
            .join(self.html_cache_path.join(path))
            .with_extension("html.hash");
        remove_file_ignore(path);
    }

    pub fn retry(&self) -> PathBufs {
        walk_glob!("{}/**/*.html.hash", self.retry_path.display())
            .par_bridge()
            .map(|path| -> Result<PathBuf> {
                let path = relative_path(&self.retry_path, &path)
                    .context("Failed to convert path")?
                    .with_extension("");
                Ok(path)
            })
            .filter_map(log_err_or_ok)
            .collect()
    }
}

fn load_hashes(hash_path: &Path, hash_cache_path: &Path, ext: &str) -> HashMap<PathBuf, Hash> {
    walk_glob!("{}/**/*{ext}.hash", hash_cache_path.display())
        .par_bridge()
        .map(|path| {
            std::fs::read_to_string(&path)
                .context("Failed to read hash file")
                .and_then(|hash| {
                    let hash = Hash::from_str(&hash).context("Failed to parse hash")?;
                    let mut path =
                        relative_path(hash_path, &path).context("Failed to convert path")?;
                    path.set_extension("");
                    Ok((path, hash))
                })
        })
        .filter_map(log_err_or_ok)
        .collect()
}

fn hash_pattern(pattern: &str) -> Vec<(PathBuf, Hash)> {
    walk_glob!("{pattern}")
        .par_bridge()
        .filter(|it| {
            it.extension()
                .and_then(|it| it.to_str())
                .map(|it| !it.ends_with("~"))
                .unwrap_or(true)
        })
        .filter_map(|path| {
            let hash = compute_hash(&path)?;
            Some((path, hash))
        })
        .collect()
}

fn compute_hash(path: &Path) -> Option<Hash> {
    let file = File::open(path).ok()?;
    let mmap = unsafe { Mmap::map(&file).ok()? };

    let mut hasher = Hasher::new();
    hasher.update(&mmap);
    Some(hasher.finalize())
}
fn refresh(
    hash_path: &Path,
    retry_path: Option<&Path>,
    hash_cache: &mut HashMap<PathBuf, Hash>,
    hash_new: HashMap<PathBuf, Hash>,
) -> Result<(PathBufs, PathBufs)> {
    // Deleted Paths
    let mut deleted_paths: PathBufs = HashSet::new();
    {
        let mut temp_cache = HashMap::new();
        hash_cache.drain().for_each(|(path, hash)| {
            if !hash_new.contains_key(&path) {
                deleted_paths.insert(path);
            } else {
                temp_cache.insert(path, hash);
            }
        });
        hash_cache.extend(temp_cache);
    }

    let updated: Vec<(PathBuf, Hash)> = hash_new
        .into_iter()
        .filter_map(|(path, hash)| match hash_cache.get(&path) {
            Some(old) if old == &hash => None, // no change
            _ => Some((path, hash)),           // changed or new
        })
        .collect();

    write_cache(hash_path, retry_path, &updated, &deleted_paths)?;

    let updated_paths: PathBufs = updated.into_iter().map(|(path, _)| path).collect();

    Ok((updated_paths, deleted_paths))
}

fn write_cache(
    hash_path: &Path,
    retry_path: Option<&Path>,
    updated: &Vec<(PathBuf, Hash)>,
    deleted: &PathBufs,
) -> Result<()> {
    updated
        .par_iter()
        .map(|(path, hash)| {
            let content = hash.to_hex().to_string();
            let mut path = path.clone();
            path.add_extension("hash");
            let hash_path = hash_path.join(&path);
            if let Some(retry_path) = retry_path {
                let retry_hash_path = retry_path.join(&path);
                remove_file_ignore(retry_hash_path);
            }
            write_into_file(hash_path, &content, "hash")
        })
        .for_each(log_err);
    deleted
        .par_iter()
        .map(|path| {
            let mut path = path.clone();
            path.add_extension("hash");
            let hash_path = hash_path.join(&path);
            if let Some(retry_path) = retry_path {
                let retry_hash_path = retry_path.join(&path);
                remove_file_ignore(retry_hash_path);
            }
            remove_file(hash_path, "hash")
        })
        .for_each(log_err);
    Ok(())
}
