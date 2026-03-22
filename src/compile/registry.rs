use crate::config::TypsiteConfig;
use crate::util::error::log_err;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

pub type Key = Arc<str>;

pub type SlugPath = Arc<Path>;

pub type KeyMap<V> = HashMap<String, V>;

pub struct KeyRegistry {
    known_articles: KeyMap<Key>,
    article_paths: KeyMap<SlugPath>,
}

impl KeyRegistry {
    pub fn new() -> Self {
        Self {
            known_articles: KeyMap::new(),
            article_paths: KeyMap::new(),
        }
    }

    pub fn register_slug(&mut self, slug: String) -> Key {
        match self.known_articles.get(&slug) {
            Some(key) => key.clone(),
            None => {
                let key: Arc<str> = Arc::from(slug.clone());
                self.known_articles.insert(slug, key.clone());
                key
            }
        }
    }
    pub fn remove_slug(&mut self, slug: &str) {
        self.known_articles.remove(slug);
        self.article_paths.remove(slug);
    }

    pub fn register_paths<I, P>(&mut self, config: &TypsiteConfig, paths: I)
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        paths
            .into_iter()
            .map(|path| self.register_article_path(config, path.as_ref()))
            .for_each(log_err);
    }

    pub fn register_article_path(
        &mut self,
        config: &TypsiteConfig,
        path: &Path,
    ) -> Result<(Key, SlugPath)> {
        let slug = config.path_to_slug(path)?;
        let slug: Key = self.register_slug(slug);
        let path = if path.starts_with(config.html_path) {
            path.strip_prefix(config.html_path)
                .with_context(|| {
                    format!(
                        "Failed to strip html root {} from {}",
                        config.html_path.display(),
                        path.display()
                    )
                })?
                .with_extension("typ")
        } else {
            path.to_path_buf()
        };
        let arc: Arc<Path> = Arc::from(path);
        let slug_with_path = (
            slug.clone(),
            self.article_paths
                .entry(slug.to_string())
                .or_insert(arc.clone())
                .clone(),
        );
        Ok(slug_with_path)
    }

    pub fn slug(&self, slug: &str) -> Option<Key> {
        self.known_articles.get(slug).cloned()
    }

    pub fn path(&self, slug: &str) -> Option<SlugPath> {
        self.article_paths.get(slug).cloned()
    }

    pub fn know(&self, slug: String, tag: &str, from: &str) -> Result<Key> {
        self.known_articles
            .get(&slug)
            .cloned()
            .with_context(|| format!("{tag} not found: {slug} in {from}"))
    }
}
