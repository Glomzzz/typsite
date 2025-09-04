use super::{PathBufs, cache::monitor::Monitor};
use crate::{
    compile::{compile_options, init_proj_options, options::ProjOptions, proj_options},
    resource::package::install_packages,
    util::path::file_ext,
};
use anyhow::*;
use std::{collections::HashSet, path::Path};

pub struct Input<'a> {
    pub monitor: Monitor<'a>,
    pub changed_typst_paths: PathBufs,
    pub deleted_typst_paths: PathBufs,
    pub changed_config_paths: PathBufs,
    pub deleted_config_paths: PathBufs,
    pub changed_non_typst: PathBufs,
    pub deleted_non_typst: PathBufs,
    pub changed_assets: PathBufs,
    pub deleted_assets: PathBufs,
    pub retry_typst_paths: PathBufs,
    pub retry_html_paths: PathBufs,
    pub overall_compile_needed: bool,
}

pub fn initialize<'a>(
    cache_path: &Path,
    typst_path: &'a Path,
    html_cache_path: &'a Path,
    config_path: &'a Path,
    assets_path: &'a Path,
    packages_path: Option<&Path>,
) -> Result<Input<'a>> {
    // Load hash cache
    let mut monitor = Monitor::load(
        cache_path,
        config_path,
        typst_path,
        html_cache_path,
        packages_path,
    );

    // Get updated and deleted typst files
    let (all_typst_paths, mut changed_typst_paths, mut deleted_typst_paths) =
        monitor.refresh_typst()?;

    // Get updated config and deleted files
    let (changed_config_paths, deleted_config_paths) = monitor.refresh_config()?;

    // Get updated and deleted non-typst files
    let (mut changed_non_typst, mut deleted_non_typst) = monitor.refresh_non_typst()?;

    // Get retry paths
    let retry_typst_paths = monitor.retry_typsts();
    let retry_html_paths = monitor.retry_htmls();

    let mut options_changed = false;
    let mut components_changed = false;
    for path in changed_config_paths.iter() {
        if options_changed && components_changed {
            break;
        }
        let path = path.to_string_lossy();
        if path.ends_with("options.toml") {
            options_changed = true;
        } else if path.contains("components\\") {
            components_changed = true;
        }
    }

    if options_changed {
        println!("Options changed, reloading...");
    }
    if components_changed {
        println!("Components changed, reloading...");
    }

    let packages_changed = if let Some(packages_path) = packages_path {
        // Get updated cand deleted package files
        let (changed_package_paths, deleted_package_paths) =
            monitor.refresh_packages(packages_path)?;
        !changed_package_paths.is_empty() || !deleted_package_paths.is_empty()
    } else {
        false
    };
    if packages_changed {
        println!("Packages changed");
        install_packages(packages_path.unwrap()).with_context(|| "Packages installing failed")?;
        println!("Packages changed, reloading...");
    }

    init_options_toml(config_path)?;
    let lib_files = &proj_options()?.typst_lib.files;
    let lib_dirs = &proj_options()?.typst_lib.dirs;
    let libs_changed = changed_typst_paths
        .iter()
        .chain(deleted_typst_paths.iter())
        .chain(changed_non_typst.iter())
        .chain(deleted_non_typst.iter())
        .filter_map(|path| path.strip_prefix(typst_path).ok())
        .any(|path| {
            let path = path.to_string_lossy();
            lib_files.contains(path.as_str())
                || lib_dirs.iter().any(|prefix| path.starts_with(prefix))
        });

    if libs_changed {
        println!("Typst lib files changed, reloading...");
    }

    let overall_compile_needed = !cache_path.exists()
        || options_changed
        || components_changed
        || libs_changed
        || packages_changed;

    if overall_compile_needed {
        changed_typst_paths = all_typst_paths;
    }

    // Remove lib paths from changed and deleted typst paths
    fn retain_lib_paths(
        typst_path: &Path,
        paths: &mut PathBufs,
        lib_files: &HashSet<String>,
        lib_dirs: &HashSet<String>,
    ) {
        paths.retain(|path| {
            let path = path.strip_prefix(typst_path).unwrap();
            !(lib_files.contains(path.to_string_lossy().as_ref())
                || lib_dirs.iter().any(|prefix| path.starts_with(prefix)))
        });
    }
    retain_lib_paths(typst_path, &mut changed_typst_paths, lib_files, lib_dirs);
    retain_lib_paths(typst_path, &mut deleted_typst_paths, lib_files, lib_dirs);
    retain_lib_paths(typst_path, &mut changed_non_typst, lib_files, lib_dirs);
    retain_lib_paths(typst_path, &mut deleted_non_typst, lib_files, lib_dirs);

    let changed_assets = changed_config_paths
        .iter()
        .filter(|path| path.starts_with(assets_path) && file_ext(path) != Some("html".to_string()))
        .cloned()
        .collect();
    let deleted_assets = deleted_config_paths
        .iter()
        .filter(|path| path.starts_with(assets_path) && file_ext(path) != Some("html".to_string()))
        .cloned()
        .collect();



    let input = Input {
        monitor,
        changed_typst_paths,
        deleted_typst_paths,
        changed_config_paths,
        deleted_config_paths,
        changed_non_typst,
        deleted_non_typst,
        changed_assets,
        deleted_assets,
        retry_typst_paths,
        retry_html_paths,
        overall_compile_needed,
    };
    Ok(input)
}

impl<'a> Input<'a> {
    pub fn unchanged(&self) -> bool {
        self.changed_typst_paths.is_empty()
            && self.deleted_typst_paths.is_empty()
            && self.changed_config_paths.is_empty()
            && self.deleted_config_paths.is_empty()
            && self.changed_non_typst.is_empty()
            && self.deleted_non_typst.is_empty()
            // In watch mode, ignore `retry_html_paths` when determining if a file is `unchanged`
            && ( compile_options().unwrap().watch || (self.retry_html_paths.is_empty() && self.retry_typst_paths.is_empty()) )
    }
}

fn init_options_toml(config_path: &Path) -> Result<()> {
    let new_options = ProjOptions::load(config_path)?;
    init_proj_options(new_options)
}
