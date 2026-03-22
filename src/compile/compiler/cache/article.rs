use crate::compile::compiler::{ErrorArticles, PathBufs};
use crate::compile::error::{TypError, TypResult};
use crate::compile::registry::{Key, KeyRegistry};
use crate::config::TypsiteConfig;
use crate::ir::article::{Article, PureArticle};
use crate::util::error::log_err_or_ok;
use crate::util::fs::{remove_file_ignore, remove_file_log_err, write_into_file};
use crate::walk_glob;
use anyhow::{anyhow, Context};
use glob::glob;
use rayon::prelude::*;
use std::collections::hash_map::Drain;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct ArticleCache<'a> {
    cache_article_path: PathBuf,
    cache_html_path: PathBuf,
    cache: HashMap<Key, Article<'a>>,
}

impl<'a> ArticleCache<'a> {
    pub fn new(cache_path: &Path) -> ArticleCache<'a> {
        let cache_article_path = cache_path.join("article");
        let cache_html_path = cache_path.join("html");
        Self {
            cache_article_path,
            cache_html_path,
            cache: HashMap::new(),
        }
    }

    fn typ_to_html_path(&self, path: &Path) -> PathBuf {
        self.cache_html_path.join(path).with_extension("html")
    }
    fn typ_to_json_path(&self, path: &Path) -> PathBuf {
        self.cache_article_path
            .join(path)
            .with_extension("typ.json")
    }

    fn errors(
        &mut self,
        registry: &mut KeyRegistry,
        error_articles: &mut HashMap<Key, (PathBuf, TypError)>,
        failed: Vec<TypError>,
    ) {
        failed.into_iter().for_each(|err| {
            let slug = err.slug.clone();
            let Some(path) = registry.path(&slug).map(|path| path.to_path_buf()) else {
                error_articles.insert(slug.clone(), (PathBuf::from(slug.as_ref()), err));
                registry.remove_slug(&slug);
                self.cache.remove(&slug);
                return;
            };
            registry.remove_slug(&slug);
            self.cache.remove(&slug);
            let cache = self.typ_to_json_path(&path);
            remove_file_ignore(cache);
            let html = self.typ_to_html_path(&path);
            error_articles.insert(slug, (html, err));
        });
        let failed = self
            .cache
            .iter()
            .filter_map(|(slug, article)| {
                let errors: Vec<_> = error_articles
                    .keys()
                    .filter_map(|error_slug| {
                        if article.get_depending_articles().contains(error_slug) {
                            Some(anyhow!("Article {error_slug} not found"))
                        } else {
                            None
                        }
                    })
                    .collect();
                if errors.is_empty() {
                    None
                } else {
                    Some(TypError::new_with(slug.clone(), errors))
                }
            })
            .collect::<Vec<_>>();
        let _ = failed.iter().filter_map(|err| self.cache.remove(&err.slug));
        if !failed.is_empty() {
            self.errors(registry, error_articles, failed);
        }
    }

    pub fn load(
        &mut self,
        config: &'a TypsiteConfig,
        deleted: &PathBufs,
        registry: &mut KeyRegistry,
    ) -> ErrorArticles {
        deleted
            .into_par_iter()
            .map(|path| self.typ_to_json_path(path))
            .for_each(|path| remove_file_log_err(path, "delete while load cahce"));

        let pures = walk_glob!("{}/**/*.json", self.cache_article_path.display())
            .par_bridge()
            .map(|path| {
                std::fs::read_to_string(&path)
                    .context("Failed to read pure article file")
                    .and_then(|json| {
                        serde_json::from_str::<PureArticle>(&json)
                            .context("Failed to parse pure article")
                    })
            })
            .filter_map(log_err_or_ok)
            .collect::<Vec<PureArticle>>();
        registry.register_paths(config, pures.iter().map(|pure| pure.path.as_path()));

        let registry: &mut KeyRegistry = registry;
        let (articles, failed): (Vec<TypResult<_>>, Vec<TypResult<_>>) = pures
            .into_iter()
            .par_bridge()
            .map(|pure| Article::from(pure, config, registry))
            .collect::<Vec<TypResult<_>>>()
            .into_iter()
            .partition(|it| it.is_ok());

        let articles: Vec<(Key, Article<'a>)> = articles
            .into_iter()
            .filter_map(|it| it.ok().map(|article| (article.slug.clone(), article)))
            .collect();
        self.cache.extend(articles);
        let failed = failed
            .into_iter()
            .filter_map(|it| it.err())
            .collect::<Vec<_>>();

        let mut error_articles = HashMap::new();
        self.errors(registry, &mut error_articles, failed);
        error_articles
            .into_iter()
            .map(|(_, (path, err))| (path, format!("{err}")))
            .collect()
    }

    #[allow(clippy::type_complexity)]
    pub fn write_cache(
        &mut self,
        slugs: HashMap<Key, (Vec<String>, Vec<String>, Vec<String>)>,
    ) -> anyhow::Result<()> {
        for (slug, cache) in slugs {
            let article = self.cache.remove(slug.as_ref()).ok_or_else(|| {
                anyhow!("Article cache entry missing while writing cache for {slug}")
            })?;
            let path = self
                .cache_article_path
                .join(format!("{}.json", article.path.display()));
            let pure = PureArticle::from(article, cache);
            let json = serde_json::to_string::<PureArticle>(&pure)
                .with_context(|| format!("Failed to serialize pure article cache for {slug}"))?;
            write_into_file(path, &json, "cache article")
                .with_context(|| format!("Failed to write article cache for {slug}"))?;
        }
        Ok(())
    }

    pub fn get(&self, slug: &Arc<str>) -> Option<&Article<'a>> {
        self.cache.get(slug)
    }
    pub fn drain(&mut self) -> Drain<'_, Key, Article<'a>> {
        self.cache.drain()
    }

    pub fn refresh<T: IntoIterator<Item = (Key, Article<'a>)>>(
        &mut self,
        registry: &mut KeyRegistry,
        new: T,
    ) {
        new.into_iter().for_each(|(slug, article)| {
            registry.register_slug(slug.to_string());
            self.cache.insert(slug, article);
        });
    }
}
