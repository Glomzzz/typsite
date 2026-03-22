use crate::compile::error::TypError;
use crate::ir::article::sidebar::{SidebarIndexes, SidebarPos};
use crate::ir::embed::Embed;
use crate::ir::rewriter::BodyRewriter;
use crate::pass::pure::embed::EmbedBuilder;
use crate::pass::pure::rewriter::RewriterBuilder;
use crate::pass::pure::PurePassData;

#[derive(Default)]
pub struct BodyBuilder<'a> {
    pub body: Vec<String>,
    pub rewriters: Vec<RewriterBuilder<'a>>,
    pub embeds: Vec<EmbedBuilder>,
}

impl<'a> BodyBuilder<'a> {
    pub fn new() -> Self {
        Self {
            body: Vec::new(),
            rewriters: Vec::new(),
            embeds: Vec::new(),
        }
    }
    pub fn push_plain(&mut self, plain: String) {
        self.body.push(plain);
    }

    pub fn push_rewriter(&mut self, rewriter: RewriterBuilder<'a>) {
        self.body.push(String::new());
        self.rewriters.push(rewriter);
    }
    pub fn push_embed(&mut self, embed: EmbedBuilder) {
        self.body.push(String::new());
        self.embeds.push(embed);
    }

    pub fn body_index(&self) -> usize {
        self.len() - 1
    }
    pub fn embed_index(&self) -> usize {
        self.embeds.len() - 1
    }
    pub fn rewriter_index(&self) -> usize {
        self.rewriters.len() - 1
    }

    pub fn len(&self) -> usize {
        self.body.len()
    }

    pub fn build_rewriter_attrs(&mut self, data: &PurePassData, error: &mut TypError) {
        for rewriter in self.rewriters.iter_mut() {
            error.result(rewriter.build_attr(data));
        }
    }

    pub fn build<F, E>(
        self,
        full_sidebar_mapping: &mut F,
        embed_sidebar_mapping: &mut E,
    ) -> (Vec<String>, Vec<BodyRewriter<'a>>, Vec<Embed>)
    where
        F: FnMut(SidebarPos) -> SidebarIndexes,
        E: FnMut(SidebarPos) -> SidebarIndexes,
    {
        (
            self.body,
            self.rewriters
                .into_iter()
                .map(|r| r.build(full_sidebar_mapping))
                .collect(),
            self.embeds
                .into_iter()
                .map(|e| e.build(full_sidebar_mapping, embed_sidebar_mapping))
                .collect(),
        )
    }
}
