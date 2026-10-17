use crate::compile::registry::Key;
use crate::config::TypsiteConfig;
use crate::ir::article::sidebar::SidebarIndexes;
use anyhow::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::result::Result::Ok;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RewriterType {
    Start,
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BodyRewriter<'a> {
    pub id: &'a str,
    pub rewriter_type: RewriterType,
    pub attributes: HashMap<String, String>,
    pub sidebar_indexes: SidebarIndexes,
    pub body_index: usize,
}

impl<'a> BodyRewriter<'a> {
    pub fn new(
        id: &'a str,
        rewriter_type: RewriterType,
        attributes: HashMap<String, String>,
        sidebar_indexes: SidebarIndexes,
        body_index: usize,
    ) -> Self {
        Self {
            id,
            rewriter_type,
            attributes,
            sidebar_indexes,
            body_index,
        }
    }
    pub fn from(
        self_slug: Key,
        pure: PureRewriter,
        config: &'a TypsiteConfig,
    ) -> Result<BodyRewriter<'a>> {
        let id = pure.id.as_str();
        let rewriter = config.rules.rule_name(id);
        if let Some(id) = rewriter {
            Ok(BodyRewriter {
                id,
                rewriter_type: pure.rewriter_type,
                attributes: pure.attributes,
                sidebar_indexes: pure.sidebar_pos,
                body_index: pure.body_index,
            })
        } else {
            Err(anyhow!(
                "Rewriter not found: {id} in {self_slug}, skip this rewriter."
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetaRewriter<'a> {
    pub id: &'a str,
    pub rewriter_type: RewriterType,
    pub attributes: HashMap<String, String>,
    pub body_index: usize,
}

impl<'a> MetaRewriter<'a> {
    pub fn new(
        rule_id: &'a str,
        rewriter_type: RewriterType,
        attributes: HashMap<String, String>,
        body_index: usize,
    ) -> Self {
        Self {
            id: rule_id,
            rewriter_type,
            attributes,
            body_index,
        }
    }
    pub fn from(
        self_slug: &str,
        meta_key: &str,
        pure: PureRewriter,
        config: &'a TypsiteConfig,
    ) -> Result<MetaRewriter<'a>> {
        let id = pure.id.as_str();
        let rewriter = config.rules.rule_name(id);
        if let Some(id) = rewriter {
            Ok(MetaRewriter {
                id,
                rewriter_type: pure.rewriter_type,
                attributes: pure.attributes,
                body_index: pure.body_index,
            })
        } else {
            Err(anyhow!(
                "Rewriter not found: {id} in metacontent {meta_key} in {self_slug}, skip this rewriter."
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PureRewriter {
    id: String,
    rewriter_type: RewriterType,
    attributes: HashMap<String, String>,
    sidebar_pos: SidebarIndexes,
    body_index: usize,
}

impl PureRewriter {
    #[allow(dead_code)]
    pub fn new(
        id: String,
        rewriter_type: RewriterType,
        attributes: HashMap<String, String>,
        sidebar_pos: SidebarIndexes,
        body_index: usize,
    ) -> Self {
        Self {
            id,
            rewriter_type,
            attributes,
            sidebar_pos,
            body_index,
        }
    }
}

impl From<BodyRewriter<'_>> for PureRewriter {
    fn from(rewriter: BodyRewriter) -> Self {
        Self {
            id: rewriter.id.to_string(),
            rewriter_type: rewriter.rewriter_type,
            attributes: rewriter.attributes,
            sidebar_pos: rewriter.sidebar_indexes,
            body_index: rewriter.body_index,
        }
    }
}
impl From<MetaRewriter<'_>> for PureRewriter {
    fn from(rewriter: MetaRewriter) -> Self {
        Self {
            id: rewriter.id.to_string(),
            rewriter_type: rewriter.rewriter_type,
            attributes: rewriter.attributes,
            sidebar_pos: SidebarIndexes::default(),
            body_index: rewriter.body_index,
        }
    }
}
