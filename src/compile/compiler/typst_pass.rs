use crate::compile::error::TypError;
use crate::config::TypsiteConfig;
use rayon::prelude::*;
use std::sync::Arc;
use std::{path::Path, result::Result::Ok};

use crate::util::error::TypsiteError;
use crate::util::fs::create_all_parent_dir;
use anyhow::{Context, Error};
use std::process::Command;

use super::cache::monitor::Monitor;
use super::{ErrorArticles, PathBufs};

pub fn compile_typst(
    typst: &str,
    root: &Path,
    config: &Path,
    input: &Path,
    output: &Path,
) -> anyhow::Result<()> {
    let font_path = config.join("assets/fonts");
    let font_path = font_path.as_path();
    let output = if cfg!(target_os = "windows") {
        Command::new("powershell")
            .arg(format!(
                "{typst} c {} --root {} -f=html --features html  {} --font-path {}",
                input.display(),
                root.display(),
                output.display(),
                font_path.display()
            ))
            .output()
            .with_context(|| format!("Typst compile to HTML failed: {}", input.display()))?
    } else {
        create_all_parent_dir(output)?;
        Command::new(typst)
            .arg("c")
            .arg(input)
            .arg("--root")
            .arg("root")
            .arg("-f=html")
            .arg("--features")
            .arg("html")
            .arg("--input")
            .arg("html-frames=true")
            .arg("--font-path")
            .arg(font_path.display().to_string())
            .arg(output)
            .output()
            .with_context(|| format!("Typst compile to HTML failed: {}", input.display()))?
    };
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(Error::new(TypsiteError::Typst(stderr.to_string())))
    }
}

pub fn compile_typsts(
    typst: &str,
    config: &TypsiteConfig<'_>,
    monitor: &mut Monitor,
    typst_path: &Path,
    config_path: &Path,
    html_cache_path: &Path,
    changed_typst_paths: &PathBufs,
    retry_typst_paths: PathBufs,
) -> ErrorArticles {
    changed_typst_paths
        .par_iter()
        .chain(&retry_typst_paths)
        .map(|typ_path| {
            let slug = config.path_to_slug(typ_path);
            let mut html_path = typ_path.clone();
            html_path.set_extension("html");
            let cache_output = html_cache_path.join(&html_path);
            let prepare_output = create_all_parent_dir(&cache_output);
            (
                slug,
                typ_path.clone(),
                prepare_output.and_then(|_| {
                    compile_typst(typst, typst_path, config_path, typ_path, &cache_output)
                }),
            )
        })
        .filter_map(|(slug, path, res)| {
            let error = if let Ok(slug) = slug {
                res.err().map(|err| {
                    let err = TypError::new_with(Arc::from(slug), vec![err]);
                    (path.clone(), format!("{err}"))
                })
            } else if let Err(err) = slug {
                Some((path.clone(), format!("{err}")))
            } else {
                None
            };
            if error.is_none() {
                monitor.remove_retry_hash(&path);
            }
            error
        })
        .collect()
}
