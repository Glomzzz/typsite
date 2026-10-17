//! Derive relationships from the current articles and validate recursion before
//! any render-time lazy initialization. Ordinary citation cycles are legal.

use super::registry::Key;
use crate::config::TypsiteConfig;
use crate::ir::article::Article;
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub struct ResolvedSite<'a> {
    pub articles: HashMap<Key, Article<'a>>,
    pub metadata_order: Vec<Key>,
}

impl<'a> ResolvedSite<'a> {
    pub fn resolve(
        config: &TypsiteConfig<'_>,
        mut articles: HashMap<Key, Article<'a>>,
    ) -> Result<Self> {
        let mut parents: BTreeMap<Key, BTreeSet<Key>> = BTreeMap::new();
        let mut backlinks: BTreeMap<Key, BTreeSet<Key>> = BTreeMap::new();
        for (slug, article) in &articles {
            let node = article.get_meta_node();
            for child in &node.children {
                ensure!(
                    articles.contains_key(child),
                    "Embedded article {child} not found in {slug}"
                );
                parents
                    .entry(child.clone())
                    .or_default()
                    .insert(slug.clone());
            }
            if let Some(parent) = &node.parent {
                ensure!(
                    articles.contains_key(parent),
                    "Parent {parent} not found in {slug}"
                );
                parents
                    .entry(slug.clone())
                    .or_default()
                    .insert(parent.clone());
            }
            for cited in &node.references {
                ensure!(
                    articles.contains_key(cited),
                    "Cited article {cited} not found in {slug}"
                );
                backlinks
                    .entry(cited.clone())
                    .or_default()
                    .insert(slug.clone());
            }
        }
        let default_parent = config
            .options
            .project
            .default_metadata
            .graph
            .parent
            .as_ref()
            .map(|slug| Key::from(config.format_slug(slug)));
        if let Some(parent) = &default_parent {
            ensure!(
                articles.contains_key(parent),
                "default_metadata.graph.parent not found: {parent}"
            );
        }
        for (slug, article) in &mut articles {
            let node = article.get_mut_meta_node();
            node.parents.clear();
            node.backlinks.clear();
            if let Some(derived) = parents.get(slug) {
                // Explicit parent wins. Otherwise use a stable inferred parent.
                if node.parent.is_none() {
                    node.parent = derived.first().cloned();
                }
                node.parents.extend(derived.iter().cloned());
            }
            node.backlinks
                .extend(backlinks.get(slug).into_iter().flatten().cloned());
        }
        let embeds = articles
            .iter()
            .map(|(slug, article)| {
                (
                    slug.clone(),
                    article.get_meta_node().children.iter().cloned().collect(),
                )
            })
            .collect();
        dependency_order("embed", &embeds)?;
        let metadata = articles
            .iter()
            .map(|(slug, article)| {
                let mut dependencies: BTreeSet<Key> =
                    article.metadata_dependencies().into_iter().collect();
                if let Some(parent) = article
                    .get_meta_node()
                    .parent
                    .as_ref()
                    .or(default_parent.as_ref())
                    .filter(|parent| *parent != slug)
                {
                    dependencies.insert(parent.clone());
                }
                (slug.clone(), dependencies)
            })
            .collect();
        let metadata_order = dependency_order("parent/metadata", &metadata)?;
        Ok(Self {
            articles,
            metadata_order,
        })
    }
}

/// Iterative DFS gives deterministic dependency-first order and a useful cycle
/// path without using the call stack on very large graphs.
fn dependency_order(kind: &str, graph: &BTreeMap<Key, BTreeSet<Key>>) -> Result<Vec<Key>> {
    let mut done = BTreeSet::new();
    let mut order = Vec::new();
    for start in graph.keys() {
        if done.contains(start) {
            continue;
        }
        let mut active = BTreeSet::new();
        let mut stack = vec![(start.clone(), false)];
        let mut path = Vec::new();
        while let Some((node, exiting)) = stack.pop() {
            if exiting {
                active.remove(&node);
                path.pop();
                done.insert(node.clone());
                order.push(node);
                continue;
            }
            if done.contains(&node) {
                continue;
            }
            if active.contains(&node) {
                path.push(node);
                anyhow::bail!(
                    "Recursive {kind} graph: {}",
                    path.iter()
                        .map(|slug: &Key| slug.as_ref())
                        .collect::<Vec<_>>()
                        .join(" -> ")
                );
            }
            active.insert(node.clone());
            path.push(node.clone());
            stack.push((node.clone(), true));
            for dependency in graph.get(&node).into_iter().flatten().rev() {
                ensure!(
                    graph.contains_key(dependency),
                    "Missing {kind} dependency {dependency} in {node}"
                );
                stack.push((dependency.clone(), false));
            }
        }
    }
    Ok(order)
}
