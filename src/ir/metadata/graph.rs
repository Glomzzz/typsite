use crate::compile::{
    error::{TypError, TypResult},
    registry::{Key, KeyRegistry},
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, PartialEq, Clone)]
pub struct MetaNode {
    // Article Slug
    pub slug: Key,
    // Parent article slug
    pub parent: Option<Key>,
    // Parent article slugs
    pub parents: HashSet<Key>,
    // Articles that link to this article
    pub backlinks: HashSet<Key>,
    // Articles that are cited in this article
    pub references: HashSet<Key>,
    // Articles that are embedded in this article
    pub children: HashSet<Key>,
}

impl MetaNode {
    pub fn from(slug: Key, pure: PureMetaNode, registry: &KeyRegistry) -> TypResult<MetaNode> {
        let mut err = TypError::new(slug.clone());
        let parent = pure
            .parent
            .and_then(|parent| err.ok(registry.know(parent, "Parent", slug.as_ref())));
        let parents = pure
            .parents
            .into_iter()
            .map(|p| err.ok(registry.know(p, "Parent", slug.as_ref())))
            .collect::<Vec<Option<_>>>();
        let backlinks = pure
            .backlinks
            .into_iter()
            .map(|p| err.ok(registry.know(p, "Backlinks", slug.as_ref())))
            .collect::<Vec<Option<_>>>();
        let references = pure
            .cited
            .into_iter()
            .map(|p| err.ok(registry.know(p, "References", slug.as_ref())))
            .collect::<Vec<Option<_>>>();
        let children = pure
            .children
            .into_iter()
            .map(|p| err.ok(registry.know(p, "Children", slug.as_ref())))
            .collect::<Vec<Option<_>>>();
        err.err_or(|| MetaNode {
            slug,
            parent,
            parents: parents.into_iter().flatten().collect(),
            backlinks: backlinks.into_iter().flatten().collect(),
            references: references.into_iter().flatten().collect(),
            children: children.into_iter().flatten().collect(),
        })
    }
    pub fn refs_and_backlinks(&self) -> HashSet<&Key> {
        self.references
            .iter()
            .chain(self.backlinks.iter())
            .collect::<HashSet<_>>()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PureMetaNode {
    pub parent: Option<String>,
    pub parents: HashSet<String>,
    pub backlinks: HashSet<String>,
    pub cited: HashSet<String>,
    pub children: HashSet<String>,
}

impl From<MetaNode> for PureMetaNode {
    fn from(node: MetaNode) -> Self {
        Self {
            parent: node.parent.map(|s| s.to_string()),
            parents: node.parents.into_iter().map(|s| s.to_string()).collect(),
            backlinks: node.backlinks.into_iter().map(|s| s.to_string()).collect(),
            cited: node.references.into_iter().map(|s| s.to_string()).collect(),
            children: node.children.into_iter().map(|s| s.to_string()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_serialize_and_de() {
        let metadata = PureMetaNode {
            parent: Some("parent".to_string()),
            parents: vec!["parent".to_string()].into_iter().collect(),
            backlinks: vec!["test1".to_string(), "test2".to_string()]
                .into_iter()
                .collect(),
            cited: HashSet::new(),
            children: vec!["test3".to_string(), "test4".to_string()]
                .into_iter()
                .collect(),
        };
        let json = serde_json::to_string(&metadata).unwrap();
        let metadata_de = serde_json::from_str(&json).unwrap();
        assert_eq!(metadata, metadata_de)
    }
}
