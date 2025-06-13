use crate::compile::compiler::{ErrorArticles, PathBufs};
use crate::compile::error::{TypError, TypResult};
use crate::compile::registry::{Key, KeyRegistry};
use crate::config::TypsiteConfig;
use crate::ir::article::{Article, PureArticle};
use crate::util::error::{log_err, log_err_or_ok};
use crate::util::fs::{remove_file_ignore, write_into_file};
use crate::walk_glob;
use anyhow::{Context, anyhow};
use glob::glob;
use rayon::prelude::*;
use std::collections::hash_map::Drain;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ArticleCache<'a> {
    cache_article_path: PathBuf,
    pub cache: HashMap<Key, Article<'a>>,
}

impl<'a> ArticleCache<'a> {
    pub fn new(cache_path: &Path) -> ArticleCache<'a> {
        let cache_article_path = cache_path.join("article");
        Self {
            cache_article_path,
            cache: HashMap::new(),
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
            .map(|path| path.with_extension("json"))
            .for_each(|path| remove_file_ignore(&path));

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
        fn errors(
            cache: &mut HashMap<Key, Article<'_>>,
            registry: &mut KeyRegistry,
            error_articles: &mut HashMap<Key, (PathBuf, TypError)>,
            failed: Vec<TypError>,
        ) {
            failed.into_iter().for_each(|err| {
                let slug = err.slug.clone();
                let path = registry.path(&slug).unwrap().to_path_buf();
                registry.remove_slug(&slug);
                cache.remove(&slug);
                error_articles.insert(slug, (path, err));
            });
            let failed = cache
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
            let _ = failed.iter().filter_map(|err| cache.remove(&err.slug));
            if !failed.is_empty() {
                errors(cache, registry, error_articles, failed);
            }
        }
        errors(&mut self.cache, registry, &mut error_articles, failed);
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
        slugs
            .into_iter()
            .map(|(slug, cache)| {
                let article = self.cache.remove(slug.as_str()).expect("Article not found");
                let path = self
                    .cache_article_path
                    .join(format!("{}.json", article.path.display()));
                let pure = PureArticle::from(article, cache);
                serde_json::to_string::<PureArticle>(&pure)
                    .context("Failed to serialize pure article")
                    .map(|json| (path, json))
            })
            .filter_map(log_err_or_ok)
            .collect::<Vec<(PathBuf, String)>>()
            .into_iter()
            .par_bridge()
            .map(|(path, json)| write_into_file(path, &json, "cache article"))
            .for_each(log_err);
        Ok(())
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
