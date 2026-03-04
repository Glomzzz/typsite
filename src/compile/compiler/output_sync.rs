use super::cache::monitor::Monitor;
use super::site_output::GeneratedFile;
use super::{ErrorArticles, PathBufs, UpdatedPages};
use crate::util::error::log_err;
use crate::util::fs::{remove_file, write_into_file};
use crate::util::path::relative_path;
use anyhow::{Ok, *};
use rayon::prelude::*;
use std::fs;
use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
};

pub struct Output<'a> {
    pub monitor: Monitor<'a>,
    pub assets_path: &'a Path,
    pub typst_path: &'a Path,
    pub html_cache_path: &'a Path,
    pub output_path: &'a Path,
    pub updated_pages: UpdatedPages<'a>,
    pub deleted_pages: PathBufs,
    pub generated_files: Vec<GeneratedFile>,
    pub generated_removed: Vec<PathBuf>,
    pub proj_options_errors: Vec<String>,
    pub error_articles: ErrorArticles,
    pub changed_non_typst: PathBufs,
    pub deleted_non_typst: PathBufs,
    pub changed_assets: PathBufs,
    pub deleted_assets: PathBufs,
}
impl<'a> Output<'a> {
    fn unchanged(&self) -> bool {
        self.updated_pages.is_empty()
            && self.deleted_pages.is_empty()
            && self.generated_files.is_empty()
            && self.generated_removed.is_empty()
            && self.error_articles.is_empty()
            && self.changed_non_typst.is_empty()
            && self.deleted_non_typst.is_empty()
            && self.changed_assets.is_empty()
            && self.deleted_assets.is_empty()
    }
}

// Return error paths
pub fn sync_files_to_output<'a>(output: Output<'a>) {
    let unchanged = output.unchanged();
    let Output {
        monitor,
        assets_path,
        typst_path,
        html_cache_path,
        output_path,
        updated_pages,
        deleted_pages,
        generated_files,
        generated_removed,
        proj_options_errors,
        error_articles,
        changed_non_typst,
        deleted_non_typst,
        changed_assets,
        deleted_assets,
    } = output;
    if !proj_options_errors.is_empty() {
        println!(
            "Project options.toml errors:\n    {}",
            proj_options_errors.join("\n    ")
        );
    }
    if unchanged {
        return;
    }
    println!("Output:");
    sync_files(
        typst_path,
        output_path,
        changed_non_typst,
        deleted_non_typst,
    );
    sync_files(assets_path, output_path, changed_assets, deleted_assets);
    write_pages(&monitor, typst_path, output_path, updated_pages);
    remove_pages(typst_path, output_path, deleted_pages);
    write_generated(output_path, generated_files);
    remove_generated(output_path, generated_removed);
    remove_errors(
        monitor,
        error_articles,
        html_cache_path,
        typst_path,
        output_path,
    );
}

fn write_pages(monitor: &Monitor, typst_path: &Path, output_path: &Path, output: UpdatedPages) {
    output
        .into_iter()
        .map(|(typ_path, html)| {
            monitor.remove_retry_hash(&typ_path);
            let html_path = relative_path(typst_path, &typ_path)
                .map(|it| it.with_extension("html"))
                .unwrap();
            let output_path = output_path.join(html_path);
            if output_path.exists() {
                println!("  ∓ {output_path:#?}");
            } else {
                println!("  + {output_path:#?}");
            }
            write_into_file(output_path, &html.to_html(), "")
        })
        .for_each(log_err);
}
fn remove_pages(typst_path: &Path, output_path: &Path, deleted_pages: PathBufs) {
    deleted_pages
        .into_par_iter()
        .map(|path| remove_output(typst_path, &path.with_extension("html"), output_path))
        .for_each(log_err);
}

fn write_generated(output_path: &Path, generated_files: Vec<GeneratedFile>) {
    generated_files
        .into_iter()
        .map(|file| {
            let output_path = output_path.join(&file.path);
            let exists = output_path.exists();
            let result = write_into_file(&output_path, &file.content, "generated file");
            if exists {
                println!("  ∓ {output_path:#?}");
            } else {
                println!("  + {output_path:#?}");
            }
            result
        })
        .for_each(log_err);
}

fn remove_generated(output_path: &Path, generated_removed: Vec<PathBuf>) {
    generated_removed
        .into_iter()
        .map(|path| {
            let output = output_path.join(&path);
            if !output.exists() {
                return Ok(());
            }
            let result = remove_file(&output, "generated file");
            println!("  - {output:#?}");
            result
        })
        .for_each(log_err);
}

fn sync_files(from: &Path, to: &Path, updated: PathBufs, deleted: PathBufs) {
    updated
        .into_par_iter()
        .map(|path| copy_to_output(from, &path, to))
        .for_each(log_err);

    deleted
        .into_par_iter()
        .map(|path| remove_output(from, &path, to))
        .for_each(log_err);
}

fn copy_to_output(parent: &Path, file: &Path, output_path: &Path) -> Result<()> {
    let file_path = relative_path(parent, file)?;
    let output_path = output_path.join(&file_path);
    if let Some(parent) = output_path.parent() {
        create_dir_all(parent).with_context(|| {
            format!("Create directory failed while creating file: {output_path:#?}")
        })?;
    }
    let exists = output_path.exists();
    fs::copy(file, &output_path)
        .with_context(|| format!("Copy {file:#?} to {output_path:#?}  failed."))?;
    if exists {
        println!("  ∓ {output_path:#?}");
    } else {
        println!("  + {output_path:#?}");
    }
    Ok(())
}

fn remove_output(parent: &Path, file: &Path, output_path: &Path) -> Result<()> {
    let file_path =
        relative_path(parent, file).with_context(|| format!("Remove file {file:#?} failed."))?;
    let output = output_path.join(&file_path);
    if !output.exists() {
        println!("  ? {output:#?}");
        return Ok(());
    }
    remove_file(&output, "output")?;
    println!("  - {output:#?}");
    // check if the dir is empty, if it is, remove the dir
    let mut parent = output.parent().unwrap();
    while parent != output {
        if fs::read_dir(parent)?.next().is_none() {
            fs::remove_dir(parent)?;
            parent = parent.parent().unwrap();
        } else {
            break;
        }
    }
    Ok(())
}

fn remove_errors(
    monitor: Monitor,
    error_articles: ErrorArticles,
    html_cache_path: &Path,
    typst_path: &Path,
    output_path: &Path,
) {
    error_articles
        .into_iter()
        .map(|(path, error)| {
            monitor.retry_next_time(&path);
            let mut cache_html_path = if !path.starts_with(html_cache_path) {
                html_cache_path.join(path)
            } else {
                path
            };
            cache_html_path.set_extension("html");
            let html_path = relative_path(html_cache_path, &cache_html_path).unwrap();
            let result = remove_output(typst_path, &html_path, output_path);
            eprintln!("{error}");
            result
        })
        .for_each(log_err)
}
