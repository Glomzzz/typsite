use crate::ir::article::sidebar::{SidebarIndexes, SidebarPos};
use crate::ir::rewriter::{BodyRewriter, MetaRewriter, RewriterType};
use crate::pass::pure::PurePassData;
use anyhow::Context;
use std::collections::HashMap;
use std::mem;

pub struct RewriterBuilder<'a> {
    id: &'a str,
    rewriter_type: RewriterType,
    attributes: HashMap<String, String>,
    sidebar_pos: Option<SidebarPos>,
    body_index: usize,
}

impl<'a> RewriterBuilder<'a> {
    pub fn new(
        rule_id: &'a str,
        rewriter_type: RewriterType,
        attributes: HashMap<String, String>,
        sidebar_pos: Option<SidebarPos>,
        body_index: usize,
    ) -> Self {
        Self {
            id: rule_id,
            rewriter_type,
            attributes,
            body_index,
            sidebar_pos,
        }
    }

    pub fn build_meta(&self) -> MetaRewriter<'a> {
        MetaRewriter::new(
            self.id,
            self.rewriter_type.clone(),
            self.attributes.clone(),
            self.body_index,
        )
    }

    pub fn build_attr(&mut self, data: &PurePassData) -> anyhow::Result<()> {
        let rule = data.config.rules.get(self.id).with_context(|| {
            format!(
                "No rewrite rule named {} while building attributes",
                self.id
            )
        })?;
        let attribute = mem::take(&mut self.attributes);
        self.attributes = rule.pass.build_attr(attribute, data)?;
        Ok(())
    }

    pub fn build<F>(self, f: &mut F) -> BodyRewriter<'a>
    where
        F: FnMut(SidebarPos) -> SidebarIndexes,
    {
        let index = self.sidebar_pos.map(f).unwrap_or_default();
        BodyRewriter::new(
            self.id,
            self.rewriter_type,
            self.attributes,
            index,
            self.body_index,
        )
    }
}
