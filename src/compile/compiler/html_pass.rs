use crate::compile::registry::KeyRegistry;
use crate::config::TypsiteConfig;
use crate::ir::article::Article;
use crate::pass::pass_pure;
use anyhow::*;
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::result::Result::Ok;

use super::cache::article::ArticleCache;
use super::ErrorArticles;

pub fn pass_html<'b, 'a: 'b>(
    config: &'a TypsiteConfig<'a>,
    cache: &'b ArticleCache<'a>,
    registry: &mut KeyRegistry,
    changed_html_paths: &mut Vec<PathBuf>,
) -> (Vec<Article<'a>>, ErrorArticles) {
    // Partition into success and errors
    let (success, errors): (Vec<_>, Vec<_>) = changed_html_paths
        .iter()
        .map(|html_path| {
            registry
                .register_article_path(config, html_path)
                .map(|(slug, path)| (slug, path, html_path))
        })
        .collect::<Vec<Result<_>>>()
        .into_iter()
        .enumerate()
        .par_bridge() // until here, we use rayon to
        // parallelize the pure pass
        .map(|(i, result)| match result {
            Ok((slug, typst_path, html_path)) => {
                let result = fs::read_to_string(html_path)
                    .with_context(|| format!("Read file {html_path:?} failed."))
                    .map(|html| {
                        let cache = cache.get(&slug);
                        pass_pure(config, registry, typst_path, slug.clone(), cache, &html)
                    });
                (i, result)
            }
            Err(e) => (i, Err(e)),
        })
        .partition(|(_, res)| !matches!(res, Err(_) | Ok(Err(_)))); // partition into success and errors

    let mut error_articles = Vec::new();
    // Report errors and remove them from Article Manager
    let mut error_indexes = Vec::new();
    errors.into_iter().for_each(|(index, err)| match err {
        Err(err) => {
            error_indexes.push((index, format!("{err:#?}")));
        }
        Ok(Err(err)) => {
            let slug = &err.slug;
            registry.remove_slug(slug);
            error_indexes.push((index, format!("{err}")))
        }
        Ok(Ok(_)) => {
            error_indexes.push((
                index,
                "Unexpected success entry reached error partition".into(),
            ));
        }
    });
    // Remove error indexes
    error_indexes.sort_by_key(|(index, _)| *index);
    error_indexes
        .into_iter()
        .rev() // Sort in reverse order to remove from the end
        .for_each(|(index, error)| {
            let path = changed_html_paths.remove(index);
            error_articles.push((path, error))
        });
    // Return only successful results
    let articles = success
        .into_iter()
        .filter_map(|(_, res)| match res {
            Ok(Ok(article)) => Some(article),
            _ => None,
        })
        .collect();
    (articles, error_articles)
}
