use crate::compile::compiler::cache::article::ArticleCache;
use crate::compile::compiler::cache::dep::RevDeps;
use crate::compile::options::CompileOptions;
use crate::compile::registry::KeyRegistry;
use crate::config::TypsiteConfig;
use crate::util::html::OutputHtml;
use crate::util::path::format_path;
use analysis::*;
use anyhow::*;
use html_pass::pass_html;
use initializer::{Input, initialize};
use output_sync::{Output, sync_files_to_output};
use page_composer::{PageData, compose_pages};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::result::Result::Ok;
use std::sync::Arc;
use typst_pass::compile_typsts;

use super::watch::watch;
use super::{init_compile_options, proj_options};

mod analysis;
mod html_pass;
mod initializer;
mod output_sync;
mod page_composer;
mod typst_pass;

mod cache {
    pub mod article;
    pub mod dep;
    pub mod monitor;
}

type PathBufs = HashSet<PathBuf>;
type ErrorArticles = Vec<(PathBuf, String)>;
type UpdatedPages<'a> = Vec<(Arc<Path>, OutputHtml<'a>)>;

pub struct Compiler {
    typst_path: PathBuf,            // Typst root
    html_cache_path: PathBuf, // Typst-export-html path (in which are raw typst-html-export files)
    config_path: PathBuf,     // Config root
    pub(crate) cache_path: PathBuf, // Cache root
    pub(crate) output_path: PathBuf,
    assets_path: PathBuf, // Output
}

impl Compiler {
    pub fn new(
        options: CompileOptions,
        cache_path: PathBuf,
        config_path: PathBuf,
        typst_path: PathBuf,
        output_path: PathBuf,
    ) -> Result<Self> {
        init_compile_options(options)?;
        let cache_path = format_path(cache_path);
        let html_cache_path = cache_path.join("html");
        let config_path = format_path(config_path);
        let assets_path = config_path.join("assets");
        let typst_path = format_path(typst_path);
        let output_path = format_path(output_path);
        Ok(Self {
            html_cache_path,
            cache_path,
            typst_path,
            config_path,
            assets_path,
            output_path,
        })
    }
    pub async fn watch(self, host: String, port: u16) -> Result<()> {
        watch(self, host, port).await
    }
    pub fn compile(&self) -> Result<bool> {
        //1. Initialize input & config
        let input = initialize(
            &self.cache_path,
            &self.typst_path,
            &self.html_cache_path,
            &self.config_path,
            &self.assets_path,
        )?;
        // If all files are not changed, return
        if input.unchanged() {
            return Ok(false);
        } else if !input.overall_compile_needed {
            println!("Files changed, compiling...");
        }
        let Input {
            mut monitor,
            config,
            changed_typst_paths,
            deleted_typst_paths,
            changed_config_paths,
            changed_non_typst,
            deleted_non_typst,
            changed_assets,
            deleted_assets,
            retry_html_paths,
            overall_compile_needed,
            ..
        } = input;

        let mut registry = KeyRegistry::new();

        // Article Manager, which manages all articles' slugs and paths
        let mut article_cache = ArticleCache::new(&self.cache_path);

        if overall_compile_needed {
            registry.register_paths(&config, changed_typst_paths.iter());
        }
        registry.register_paths(&config, retry_html_paths.iter());

        let error_cache_articles = article_cache.load(&config, &deleted_typst_paths, &mut registry);

        let proj_options_errors = verify_proj_options(&config, &registry)?;

        //2. Export typst as HTML
        // Only compile updated typst files into html
        let error_typst_articles = compile_typsts(
            &config,
            &self.typst_path,
            &self.html_cache_path,
            &changed_typst_paths,
        );

        let mut changed_html_paths =
            monitor.refresh_html(&deleted_typst_paths, overall_compile_needed)?;

        changed_html_paths.extend(retry_html_paths);

        //3. Pass HTML
        // Pass updated html files
        let (changed_articles, error_pending_articles) =
            pass_html(&config, &mut registry, &mut changed_html_paths);

        let changed_article_slugs = changed_articles
            .iter()
            .map(|article| article.slug.clone())
            .collect::<HashSet<_>>();

        // Collect all updated articles
        let mut loaded_articles = article_cache
            .drain() // Drain all articles from Article Manager ( for a simpler lifetime)
            .chain(changed_articles.into_iter().map(|a| (a.slug.clone(), a)))
            .collect::<HashMap<_, _>>();

        //4. Analyse articles
        // Record parents and backlinks
        let (parents, backlinks) =
            analyse_parents_and_backlinks(loaded_articles.values().collect());

        // Update parents and backlinks into all loaded articles
        apply_parents_and_backlinks(&mut loaded_articles, parents, backlinks);

        // Load Reverse Dependency Cache
        let mut rev_dep = RevDeps::load(
            &config,
            &self.cache_path,
            &deleted_typst_paths,
            &mut registry,
        );

        // Refresh Dependency Cache
        // in which we record all the dependencies(with its exactly indexes) of each article,
        // and the Reverse Dependencies of each file path are collected. ( Reverse Dependencies = Map<Path -> The files that depend on this file>)
        rev_dep.refresh(&config, &registry, &loaded_articles);

        // 5. Compose pages
        let PageData {
            updated_pages,
            cache,
            error_pages,
        } = compose_pages(
            &config,
            changed_article_slugs,
            changed_typst_paths,
            &changed_config_paths,
            &loaded_articles,
            rev_dep,
            overall_compile_needed,
        )?;

        let updated = !loaded_articles.is_empty();
        // 6. Update cache
        article_cache.refresh(&mut registry, loaded_articles);
        article_cache.write_cache(cache)?;

        // 7. Sync files to output
        let deleted_pages = deleted_typst_paths;

        let mut error_articles = Vec::new();
        error_articles.extend(error_typst_articles);
        error_articles.extend(error_cache_articles);
        error_articles.extend(error_pending_articles);
        error_articles.extend(error_pages);

        let output = Output {
            monitor,
            assets_path: &self.assets_path,
            typst_path: &self.typst_path,
            html_cache_path: &self.html_cache_path,
            output_path: &self.output_path,
            updated_pages,
            deleted_pages,
            proj_options_errors,
            error_articles,
            changed_non_typst,
            deleted_non_typst,
            changed_assets,
            deleted_assets,
        };

        sync_files_to_output(output);

        Ok(updated)
    }
}

fn verify_proj_options(config: &TypsiteConfig<'_>, registry: &KeyRegistry) -> Result<Vec<String>> {
    let mut errors = Vec::new();
    let options = proj_options()?;
    let parent = options.default_metadata.graph.parent.clone();
    if let Some(parent) = parent {
        let parent = config.format_slug(&parent);
        if let Err(err) = registry.know(parent, "default_metadata.graph.parent", "options.toml") {
            errors.push(format!("{err}"))
        }
    }
    Ok(errors)
}
