use super::cache::dep::RevDeps;
use crate::compile::registry::Key;
use crate::compile::compiler::PathBufs;
use crate::ir::article::Article;
use std::collections::{HashMap, HashSet};

pub(super) type Relation = HashMap<Key, HashSet<Key>>;

pub(super) fn analyse_parents_and_backlinks<'b, 'a: 'b>(
    changed_articles: Vec<&Article<'a>>,
) -> (Relation, Relation) {
    let mut parents: HashMap<Key, HashSet<Key>> = HashMap::new();
    let mut backlinks: HashMap<Key, HashSet<Key>> = HashMap::new();
    changed_articles.into_iter().for_each(|article| {
        let node = article.get_meta_node();
        for cited in &node.references {
            backlinks
                .entry(cited.clone())
                .or_default()
                .insert(article.slug.clone());
        }
        for child in &node.children {
            if parents.contains_key(child) {
                continue;
            }
            parents
                .entry(child.clone())
                .or_default()
                .insert(article.slug.clone());
        }

        if let Some(parent) = &node.parent {
            parents
                .entry(article.slug.clone())
                .or_default()
                .insert(parent.clone());
        }
    });
    (parents, backlinks)
}

pub(super) fn apply_parents_and_backlinks<'b, 'a: 'b>(
    articles: &mut HashMap<Key, Article<'a>>,
    parents: Relation,
    backlinks: Relation,
) {
    parents.into_iter().for_each(|(child, parent_slug)| {
        if let Some(child) = articles.get_mut(child.as_str()) {
            if child.get_meta_node().parent.is_none() {
                child.get_mut_meta_node().parent = parent_slug.iter().next().cloned();
            }
            child.get_mut_meta_node().parents.extend(parent_slug);
        }
    });

    backlinks.into_iter().for_each(|(cited, backlink)| {
        if let Some(cited) = articles.get_mut(&cited) {
            cited.get_mut_meta_node().backlinks.extend(backlink);
        }
    });
}
pub(super) fn analyse_slugs_to_update_and_load<'b, 'a: 'b>(
    changed_article_slugs: &HashSet<Key>,
    updated_typst_paths: &mut PathBufs,
    changed_config_paths: &PathBufs,
    loaded_articles: &HashMap<Key, Article<'a>>,
    rev_dep: &RevDeps,
) -> (HashSet<Key>, HashSet<Key>) {
    let mut slugs_to_update = HashSet::new();
    let mut slugs_to_load = HashSet::new();

    let mut slugs: HashSet<Key> = updated_typst_paths // Changed typst files
        .iter()
        .chain(changed_config_paths.iter()) // Changed config files
        .filter_map(|path| rev_dep.get(path)) // All files that depend on them need to be updated
        .flatten()
        .cloned()
        .collect();

    slugs.extend(changed_article_slugs.iter().cloned()); // If an article is changed, itself needs to be updated

    // Maybe this algorithm needs to be improved
    fn spread_parent<'b, 'a: 'b>(
        articles: &HashMap<Key, Article<'a>>,
        slug: &Key,
        update: &mut HashSet<Key>,
        updated_paths: &mut PathBufs,
    ) {
        if let Some(article) = articles.get(slug) {
            update.insert(slug.clone()); // Only the existing article is collected.
            updated_paths.insert(article.path.to_path_buf());
            // Spread all parents
            for parent in &article.get_meta_node().parents {
                if update.contains(parent) {
                    continue;
                }
                spread_parent(articles, parent, update, updated_paths);
            }
            // Spread all children (children can use parent's meta contents)
            for child in &article.get_meta_node().children {
                if update.contains(child) {
                    continue;
                }
                spread_parent(articles, child, update, updated_paths);
            }
        }
    }
    // Maybe this algorithm needs to be improved
    fn spread_child<'b, 'a: 'b>(
        articles: &HashMap<Key, Article<'a>>,
        slug: &Key,
        load: &mut HashSet<Key>,
    ) {
        if let Some(article) = articles.get(slug) {
            load.insert(slug.clone()); // Only the existing article is collected.
            for parent in &article.get_meta_node().parents {
                if load.contains(parent) {
                    continue;
                }
                spread_child(articles, parent, load);
            }
            // Spread all children
            for child in &article.get_meta_node().children {
                if load.contains(child) {
                    continue;
                }
                spread_child(articles, child, load);
            }
        }
    }

    // Spread all dependencies
    // Make sure all files that need to update are collected in slugs_need_to_update
    slugs.into_iter().for_each(|slug| {
        spread_parent(
            loaded_articles,
            &slug,
            &mut slugs_to_update,
            updated_typst_paths,
        );
        spread_child(loaded_articles, &slug, &mut slugs_to_load);
    });

    (slugs_to_update, slugs_to_load)
}
