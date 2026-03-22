use crate::compile::compiler::cache::article::ArticleCache;
use crate::compile::compiler::cache::dep::RevDeps;
use crate::compile::options::CompileOptions;
use crate::compile::registry::KeyRegistry;
use crate::config::TypsiteConfig;
use crate::util::fs::remove_dir_all;
use crate::util::html::OutputHtml;
use analysis::*;
use anyhow::*;
use html_pass::pass_html;
use initializer::{Input, initialize};
use output_sync::{Output, sync_files_to_output};
use site_output::generate_site_outputs;
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
mod site_output;
mod typst_pass;

mod cache {
    pub mod article;
    pub mod dep;
    pub mod monitor;
}

type PathBufs = HashSet<PathBuf>;
type ErrorArticles = Vec<(PathBuf, String)>;
type UpdatedPages<'a> = Vec<(Arc<Path>, OutputHtml<'a>)>;

pub fn clean_dir(path: &Path) -> Result<()> {
    if path.exists() {
        println!("  - Cleaning dir: {path:?}");
        remove_dir_all(path)?;
    }
    Ok(())
}

pub struct Compiler {
    typst_path: PathBuf,      // Typst root
    html_cache_path: PathBuf, // Typst-export-html path (in which are raw typst-html-export files)
    config_path: PathBuf,     // Config root
    assets_path: PathBuf,     // Assets
    cache_path: PathBuf,      // Cache root
    output_path: PathBuf,     // Output
    packages_path: Option<PathBuf>,// Package
    typst: String,            // Typst executable path
}

impl Compiler {
    pub fn new(
        options: CompileOptions,
        cache_path: PathBuf,
        config_path: PathBuf,
        typst_path: PathBuf,
        output_path: PathBuf,
        packages_path: Option<PathBuf>,
        typst: String
    ) -> Result<Self> {
        init_compile_options(options)?;
        let html_cache_path = cache_path.join("html");
        let assets_path = config_path.join("assets");
        Ok(Self {
            typst_path,
            html_cache_path,
            config_path,
            assets_path,
            cache_path,
            output_path,
            packages_path,
            typst
        })
    }
    pub fn output_path(&self) -> &Path {
        &self.output_path
    }
    pub fn clean(&self) -> Result<()> {
        clean_dir(&self.cache_path)?;
        clean_dir(&self.output_path)
    }
    pub async fn watch(self, host: String, port: u16) -> Result<()> {
        watch(self, host, port).await
    }
    // return (updated, no error)
    pub fn compile(&self) -> Result<(bool, bool)> {
        //1. Initialize input & config
        let input = initialize(
            &self.cache_path,
            &self.typst_path,
            &self.html_cache_path,
            &self.config_path,
            &self.assets_path,
            self.packages_path.as_deref(),
        );
        let input = match input {
            Ok(input) => input,
            Err(err) => {
                eprintln!("Error initializing compiler: {err}");
                return Ok((false, false));
            }
        };
        // If all files are not changed, return
        if input.unchanged() {
            return Ok((false, true));
        } else if !input.overall_compile_needed {
            println!("Files changed, compiling...");
        }
        let Input {
            mut monitor,
            changed_typst_paths,
            deleted_typst_paths,
            changed_config_paths,
            changed_non_typst,
            deleted_non_typst,
            changed_assets,
            deleted_assets,
            retry_typst_paths,
            retry_html_paths,
            overall_compile_needed,
            ..
        } = input;


        let config =
            TypsiteConfig::load(&self.config_path, &self.typst_path, &self.html_cache_path).with_context(|| {
                format!("Loading '{:?}' failed, try to init Typsite first by: typsite init",&self.config_path)
            })?;

        let mut registry = KeyRegistry::new();

        // Article Manager, which manages all articles' slugs and paths
        let mut article_cache = ArticleCache::new(&self.cache_path);

        if overall_compile_needed {
            registry.register_paths(&config, changed_typst_paths.iter());
        }
        registry.register_paths(&config, retry_typst_paths.iter());
        registry.register_paths(&config, retry_html_paths.iter());

        let error_cache_articles = article_cache.load(&config, &deleted_typst_paths, &mut registry);

        let proj_options_errors = verify_proj_options(&config, &registry)?;

        //2. Export typst as HTML
        // Only compile updated typst files into html
        let error_typst_articles = compile_typsts(
            &self.typst,
            &config,
            &mut monitor,
            &self.typst_path,
            &self.config_path,
            &self.html_cache_path,
            &changed_typst_paths,
            retry_typst_paths,
        );

        let mut changed_html_paths =
            monitor.refresh_html(&deleted_typst_paths, overall_compile_needed)?;

        changed_html_paths.extend(retry_html_paths);

        //3. Pass HTML
        // Pass updated html files
        let (changed_articles, error_passing_articles) = pass_html(
            &config,
            &article_cache,
            &mut registry,
            &mut changed_html_paths,
        );

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
        let generated_site = generate_site_outputs(&loaded_articles)?;

        // 6. Update cache
        article_cache.refresh(&mut registry, loaded_articles);
        article_cache.write_cache(cache)?;

        // 7. Sync files to output
        let deleted_pages = deleted_typst_paths;

        let mut error_articles = Vec::new();
        error_articles.extend(error_typst_articles);
        error_articles.extend(error_cache_articles);
        error_articles.extend(error_passing_articles);
        error_articles.extend(error_pages);

        let no_error = error_articles.is_empty();

        let output = Output {
            monitor,
            assets_path: &self.assets_path,
            typst_path: &self.typst_path,
            html_cache_path: &self.html_cache_path,
            output_path: &self.output_path,
            updated_pages,
            deleted_pages,
            generated_files: generated_site.files,
            generated_removed: generated_site.removed,
            proj_options_errors,
            error_articles,
            changed_non_typst,
            deleted_non_typst,
            changed_assets,
            deleted_assets,
        };

        sync_files_to_output(output);

        Ok((updated, no_error))
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::compile::compiler::cache::monitor::Monitor;
    use crate::compile::{
        init_compile_options, init_proj_options,
        options::{CompileOptions, ProjOptions},
    };
    use crate::config::TypsiteConfig;
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_root(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test timestamp should be available")
            .as_nanos();
        std::env::temp_dir().join(format!("typsite-{label}-{unique}"))
    }

    fn write_test_config(root: &std::path::Path, options: &str) {
        fs::create_dir_all(root.join("components/footer")).expect("footer dir");
        fs::create_dir_all(root.join("rewrite")).expect("rewrite dir");
        fs::create_dir_all(root.join("schemas")).expect("schema dir");
        fs::create_dir_all(root.join("syntaxes")).expect("syntaxes dir");
        fs::create_dir_all(root.join("themes")).expect("themes dir");
        fs::write(root.join("options.toml"), options).expect("options file");
        fs::write(
            root.join("components/section.html"),
            include_str!("../../resources/.typsite/components/section.html"),
        )
        .expect("section");
        fs::write(
            root.join("components/heading-numbering.html"),
            "<html><body>{numbering}</body></html>",
        )
        .expect("heading numbering");
        fs::write(
            root.join("components/footer.html"),
            "<html><body><footer>{references}{backlinks}</footer></body></html>",
        )
        .expect("footer");
        fs::write(
            root.join("components/footer/backlinks.html"),
            "<html><body>{backlinks}</body></html>",
        )
        .expect("backlinks");
        fs::write(
            root.join("components/footer/references.html"),
            "<html><body>{references}</body></html>",
        )
        .expect("references");
        fs::write(root.join("components/anchor_def.html"), "<html><body></body></html>")
            .expect("anchor def");
        fs::write(root.join("components/anchor_def_svg.html"), "<html><body></body></html>")
            .expect("anchor def svg");
        fs::write(root.join("components/anchor_goto.html"), "<html><body></body></html>")
            .expect("anchor goto");
        fs::write(root.join("components/anchor_goto_svg.html"), "<html><body></body></html>")
            .expect("anchor goto svg");
        fs::write(root.join("components/sidebar.html"), "<html><body>{children}</body></html>")
            .expect("sidebar");
        fs::write(root.join("components/sidebar_each.html"), "<html><body>{title}</body></html>")
            .expect("sidebar each");
        fs::write(root.join("components/embed.html"), "<html><body>{content}</body></html>")
            .expect("embed");
        fs::write(root.join("components/embed_title.html"), "<html><body>{title}</body></html>")
            .expect("embed title");
        fs::write(root.join("schemas/main.html"), "<html><body><content></content></body></html>")
            .expect("schema");
        fs::write(root.join("schemas/reference.html"), "<html><body><content></content></body></html>")
            .expect("reference schema");
    }

    fn init_options(root: &std::path::Path) {
        let proj = ProjOptions::load(root).expect("project options should load");
        init_proj_options(proj).expect("project options should initialize");
        init_compile_options(CompileOptions {
            watch: false,
            short_slug: false,
            pretty_url: true,
        })
        .expect("compile options should initialize");
    }

    pub(crate) fn run_compose_pages_collects_missing_article_as_error() {
        let root = unique_root("compose-pages-missing-article");
        let cache = root.join("cache");
        let typst = root.join("typst");
        let html = root.join("html");
        write_test_config(&root, include_str!("../../resources/.typsite/options.toml"));
        fs::create_dir_all(&cache).expect("cache dir");
        fs::create_dir_all(&typst).expect("typst dir");
        fs::create_dir_all(&html).expect("html dir");
        init_options(&root);

        let config = TypsiteConfig::load(&root, &typst, &html).expect("config should load");
        let mut registry = KeyRegistry::new();
        let rev_dep = cache::dep::RevDeps::load(&config, &cache, &HashSet::new(), &mut registry);
        let changed = HashSet::from([Arc::from("/missing")]);
        let page_data = page_composer::compose_pages(
            &config,
            changed,
            HashSet::new(),
            &HashSet::new(),
            &HashMap::new(),
            rev_dep,
            false,
        )
        .expect("compose pages should collect the missing article as an error entry");

        assert_eq!(page_data.error_pages.len(), 1);
        assert!(page_data.error_pages[0].1.contains("should be loaded before composition"));

        let _ = fs::remove_dir_all(root);
    }

    pub(crate) fn run_pass_html_collects_registry_errors_without_unreachable_panics() {
        let root = unique_root("pass-html-registry-errors");
        let typst = root.join("typst");
        let html = root.join("html");
        write_test_config(&root, include_str!("../../resources/.typsite/options.toml"));
        fs::create_dir_all(&typst).expect("typst dir");
        fs::create_dir_all(&html).expect("html dir");
        init_options(&root);

        let config = TypsiteConfig::load(&root, &typst, &html).expect("config should load");
        let invalid_html = root.join("outside.html");
        fs::write(&invalid_html, "<html><body></body></html>").expect("html file");
        let mut changed_html_paths = vec![invalid_html.clone()];
        let (articles, errors) = html_pass::pass_html(
            &config,
            &cache::article::ArticleCache::new(&root.join("cache")),
            &mut KeyRegistry::new(),
            &mut changed_html_paths,
        );

        assert!(articles.is_empty());
        assert_eq!(errors.len(), 1);
        assert!(errors[0].0.ends_with("outside.html"));
        assert!(changed_html_paths.is_empty());

        let _ = fs::remove_dir_all(root);
    }

    pub(crate) fn run_compile_typsts_reports_missing_cache_parent_without_panic() {
        let root = unique_root("compile-typsts-cache-parent");
        let cache = root.join("cache");
        let typst = root.join("typst");
        let html_cache_root = root.join("html-cache-root");
        let rendered_html = root.join("rendered-html");
        write_test_config(&root, include_str!("../../resources/.typsite/options.toml"));
        fs::create_dir_all(&cache).expect("cache dir");
        fs::create_dir_all(typst.join("nested")).expect("typst dir");
        fs::create_dir_all(&rendered_html).expect("rendered html dir");
        fs::write(typst.join("nested/article.typ"), "= Test").expect("typst file");
        fs::write(&html_cache_root, "occupied by file").expect("blocking html cache path");
        init_options(&root);

        let config = TypsiteConfig::load(&root, &typst, &rendered_html).expect("config should load");
        let mut monitor = Monitor::load(&cache, &root, &typst, &rendered_html, None);
        let errors = typst_pass::compile_typsts(
            "typst",
            &config,
            &mut monitor,
            &typst,
            &root,
            &html_cache_root,
            &HashSet::from([typst.join("nested/article.typ")]),
            HashSet::new(),
        );

        assert_eq!(errors.len(), 1);
        assert!(errors[0].1.contains("parent") || errors[0].1.contains("Not a directory"));

        let _ = fs::remove_dir_all(root);
    }

    pub(crate) fn run_initialize_reports_packages_path_and_strip_prefix_failures() {
        let root = unique_root("initialize-packages-path");
        let cache = root.join("cache");
        let typst = root.join("typst");
        let html = root.join("html");
        let assets = root.join("assets");
        let packages_dir = root.join("packages");
        write_test_config(&root, include_str!("../../resources/.typsite/options.toml"));
        fs::create_dir_all(&cache).expect("cache dir");
        fs::create_dir_all(&typst).expect("typst dir");
        fs::create_dir_all(&html).expect("html dir");
        fs::create_dir_all(&assets).expect("assets dir");
        fs::create_dir_all(packages_dir.join("broken-package")).expect("packages dir");
        fs::write(packages_dir.join("broken-package/README.txt"), "missing typst manifest")
            .expect("package marker file");

        let err = match initializer::initialize(&cache, &typst, &html, &root, &assets, Some(&packages_dir)) {
            Ok(_) => panic!("file packages path should be reported as an error"),
            Err(err) => err,
        };
        let err = err.to_string();

        assert!(err.contains("Packages installing failed") || err.contains("typst.toml"));

        let _ = fs::remove_dir_all(root);
    }

    pub(crate) fn run_generate_site_outputs_handles_empty_base_url_without_unwrap() {
        let root = unique_root("generate-site-empty-base-url");
        write_test_config(&root, include_str!("../../resources/.typsite/options.toml"));
        init_options(&root);

        let generated = site_output::generate_site_outputs(&HashMap::new())
            .expect("empty base url should disable outputs without panicking");

        assert!(generated.files.is_empty());
        assert_eq!(generated.removed.len(), 2);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn compose_pages_collects_missing_article_as_error() {
        let _guard = crate::test_lock();
        run_compose_pages_collects_missing_article_as_error();
    }

    #[test]
    fn pass_html_collects_registry_errors_without_unreachable_panics() {
        let _guard = crate::test_lock();
        run_pass_html_collects_registry_errors_without_unreachable_panics();
    }

    #[test]
    fn compile_typsts_reports_missing_cache_parent_without_panic() {
        let _guard = crate::test_lock();
        run_compile_typsts_reports_missing_cache_parent_without_panic();
    }

    #[test]
    fn initialize_reports_packages_path_and_strip_prefix_failures() {
        let _guard = crate::test_lock();
        run_initialize_reports_packages_path_and_strip_prefix_failures();
    }

    #[test]
    fn generate_site_outputs_handles_empty_base_url_without_unwrap() {
        let _guard = crate::test_lock();
        run_generate_site_outputs_handles_empty_base_url_without_unwrap();
    }
}
