use super::cache::dep::RevDeps;
use crate::compile::error::TypResult;
use crate::compile::registry::Key;
use crate::config::TypsiteConfig;
use crate::ir::article::data::GlobalData;
use crate::ir::article::dep::{Indexes, UpdatedIndex};
use crate::ir::article::sidebar::SidebarType;
use crate::ir::article::Article;
use crate::ir::embed::SectionType;
use crate::pass::pass_schema;
use anyhow::*;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::result::Result::Ok;
use std::sync::{Arc, OnceLock};

use super::{analyse_slugs_to_update_and_load, ErrorArticles, PathBufs, UpdatedPages};

pub type PageCache = HashMap<Key, (Vec<String>, Vec<String>, Vec<String>)>;

pub struct PageData<'a> {
    pub cache: PageCache,
    pub updated_pages: UpdatedPages<'a>,
    pub error_pages: ErrorArticles,
}

pub fn compose_pages<'c, 'b: 'c, 'a: 'b>(
    config: &'a TypsiteConfig<'a>,
    changed_article_slugs: HashSet<Key>,
    changed_typst_paths: PathBufs,
    changed_config_paths: &PathBufs,
    loaded_articles: &'c HashMap<Key, Article<'a>>,
    rev_dep: RevDeps,
    overall_compile_needed: bool,
) -> Result<PageData<'a>> {
    let mut updated_typst_paths = changed_typst_paths.clone();

    // Collect all slugs that need to update
    // - If a file is changed, all files that depend on it need to be updated
    // - If an article is changed, itself needs to be updated
    let (slugs_to_update, slugs_to_load) = analyse_slugs_to_update_and_load(
        &changed_article_slugs,
        &mut updated_typst_paths,
        changed_config_paths,
        loaded_articles,
        &rev_dep,
    );

    let (mut global_meta_indexes, global_body_rewrite_indexes, global_body_embed_indexes) =
        analyse_global_indexes(
            loaded_articles,
            &slugs_to_update,
            changed_typst_paths,
            changed_config_paths,
            &updated_typst_paths,
            rev_dep,
            overall_compile_needed,
        );

    let pendings = slugs_to_load
        .into_iter()
        .map(|slug| (slug, OnceLock::new()))
        .collect();

    let global_data = GlobalData::new(
        config,
        loaded_articles,
        pendings,
        global_body_rewrite_indexes,
        global_body_embed_indexes,
    );

    // update_meta_content_indexes
    slugs_to_update
        .iter()
        .map(|slug| {
            let article = global_data.article(slug).unwrap();
            let meta_rewriter_indexes = global_meta_indexes.remove(article.slug.as_ref()).unwrap();
            let meta_contents = article.get_meta_contents();
            (meta_contents, meta_rewriter_indexes)
        })
        .map(|(meta_contents, mut meta_rewriter_indexes)| {
            if meta_rewriter_indexes.is_empty() {
                for meta_key in meta_contents.keys() {
                    meta_rewriter_indexes.insert(meta_key.to_string(), Indexes::All);
                }
            }
            for (meta_key, indexes) in meta_rewriter_indexes {
                meta_contents.pass_content(&meta_key, indexes, &global_data);
            }
            meta_contents
        })
        .for_each(|meta_contents| meta_contents.init_parent(&global_data));

    let empty_pos = vec![];
    let final_cache = slugs_to_update
        .iter()
        .cloned()
        .map(|slug| (slug, OnceLock::new()))
        .collect::<HashMap<_, OnceLock<(Vec<String>, Vec<String>, Vec<String>)>>>();
    // Eval content as Pending ()
    let (output, failed): (Vec<TypResult<_>>, Vec<TypResult<_>>) = slugs_to_update
        .par_iter()
        .map(|slug| -> TypResult<(&Article, (String, String))> {
            let article = global_data.article(slug).unwrap(); // Pretty ensure that the article is valid
            let pending = article.get_pending_or_init(&global_data);
            let (content, full_sidebar, embed_sidebar) = pending.based_on(
                config,
                &global_data,
                Some(&empty_pos),
                Some(&empty_pos),
                article.get_meta_options().sidebar_type,
                SectionType::Full,
            );
            let content_str = content.join("");

            let sidebar_str = if article.get_meta_options().sidebar_type == SidebarType::All {
                full_sidebar.join("")
            } else {
                embed_sidebar.join("")
            };

            final_cache[slug]
                .set((content, full_sidebar, embed_sidebar))
                .unwrap();
            let node = &article.get_meta_node();
            if !node.backlinks.is_empty() {
                // If the article has Backlinks -> it's cited, the citing articles need the reference.
                global_data.init_reference(article, &content_str, &sidebar_str)?;
            }
            if !node.references.is_empty() {
                // If the article has References -> it's citing other articles, the cited articles need the backlink.
                global_data.init_backlink(article, &content_str, &sidebar_str)?;
            }
            Ok((article, (content_str, sidebar_str)))
        })
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|result| {
            result.and_then(|(article, (content, sidebar))| {
                let schema = article.schema;
                // Form a Page for each article
                pass_schema(
                    config,
                    schema,
                    article,
                    content.as_str(),
                    sidebar.as_str(),
                    &global_data,
                )
                .map(|html| (article.path.clone(), html))
            })
        })
        .partition(|res| res.is_ok());
    let cache: PageCache = final_cache
        .into_iter()
        .par_bridge()
        .map(|(slug, lock)| (slug, lock.into_inner().unwrap()))
        .collect();
    let updated_pages: UpdatedPages = output.into_iter().flatten().collect();
    let error_articles = failed
        .into_iter()
        .filter_map(|it| it.err())
        .map(|err| {
            let path = global_data.article(&err.slug).unwrap().path.to_path_buf();
            (path, format!("{err}"))
        })
        .collect();
    Ok(PageData {
        cache,
        updated_pages,
        error_pages: error_articles,
    })
}

fn analyse_global_indexes<'a, 'b, 'c>(
    updated_articles: &'c HashMap<Key, Article<'a>>,
    slugs_to_update: &HashSet<Key>,
    changed_typst_paths: PathBufs,
    changed_config_paths: &PathBufs,
    updated_typst_paths: &PathBufs,
    mut rev_dependency: RevDeps,
    overall_compile_needed: bool,
) -> (
    HashMap<Arc<str>, HashMap<String, Indexes>>,
    HashMap<Arc<str>, Indexes>,
    HashMap<Arc<str>, Indexes>,
)
where
    'a: 'b,
    'b: 'c,
{
    // in which we record each article's indexes where need to update
    let mut global_indexes: HashMap<Key, HashSet<UpdatedIndex>> = slugs_to_update
        .iter()
        .map(|slug| {
            let article = updated_articles.get(slug).unwrap(); // We pretty ensure that the article is valid
            let changed = changed_typst_paths.contains(article.path.as_ref()); // If the article is updated
            let indexes = if overall_compile_needed || changed {
                HashSet::new() // If it's init or updated, we need to update all indexes, which is represented by an empty set
            } else {
                updated_typst_paths // Changed typst files
                    .iter()
                    .chain(changed_config_paths.iter())
                    .filter_map(
                        |path| rev_dependency.take_dependency(slug, path),
                        // Collect all dependencies (with indexes) of the article,
                        // For each (changed) dependency, collect the indexes of the article
                    )
                    .flatten()
                    .collect::<HashSet<_>>()
            };
            (slug.clone(), indexes)
        })
        .collect();
    let mut global_meta_indexes = HashMap::new();
    let mut global_body_rewrite_indexes = HashMap::new();
    let mut global_body_embed_indexes = HashMap::new();
    updated_articles.values().for_each(|article| {
        let mut meta_rewriter_indexes: HashMap<String, Indexes> = HashMap::new();
        let mut body_rewriter_indexes = Indexes::All;
        let mut embed_indexes = Indexes::All;
        if let Some(indexes) = global_indexes.remove(article.slug.as_ref()) {
            for index in indexes {
                match index {
                    UpdatedIndex::MetaRewriter(meta_key, index) => {
                        if let Indexes::Some(indexes) = meta_rewriter_indexes
                            .entry(meta_key.to_string())
                            .or_insert(Indexes::Some(HashSet::default()))
                        {
                            indexes.insert(index);
                        }
                    }
                    UpdatedIndex::BodyRewriter(index) => {
                        if let Indexes::Some(indexes) = &mut body_rewriter_indexes {
                            indexes.insert(index);
                        } else {
                            body_rewriter_indexes = Indexes::Some([index].into_iter().collect());
                        }
                    }
                    UpdatedIndex::Embed(index) => {
                        if let Indexes::Some(indexes) = &mut embed_indexes {
                            indexes.insert(index);
                        } else {
                            embed_indexes = Indexes::Some([index].into_iter().collect());
                        }
                    }
                }
            }
        }
        global_meta_indexes.insert(article.slug.clone(), meta_rewriter_indexes);
        global_body_rewrite_indexes.insert(article.slug.clone(), body_rewriter_indexes);
        global_body_embed_indexes.insert(article.slug.clone(), embed_indexes);
    });
    (
        global_meta_indexes,
        global_body_rewrite_indexes,
        global_body_embed_indexes,
    )
}
