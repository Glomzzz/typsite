use crate::compile::error::TypError;
use crate::config::TypsiteConfig;
use rayon::prelude::*;
use std::sync::Arc;
use std::{path::Path, result::Result::Ok};

use crate::compile::typst::{CommandTypst, ExportRequest, TypstRunner};
use crate::util::fs::create_all_parent_dir;

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
    CommandTypst::new(typst)
        .export(&ExportRequest {
            root,
            input,
            output,
            font_path: &font_path,
        })
        .map(|_| ())
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
            let cache_output = typ_path
                .strip_prefix(typst_path)
                .map(|path| html_cache_path.join(path).with_extension("html"))
                .map_err(anyhow::Error::from);
            (
                slug,
                typ_path.clone(),
                cache_output.and_then(|cache_output| {
                    create_all_parent_dir(&cache_output)?;
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
