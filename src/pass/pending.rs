use crate::compile::registry::Key;
use crate::ir::article::data::GlobalData;
use crate::ir::article::dep::Indexes;
use crate::ir::article::sidebar::{Pos, Sidebar, SidebarIndexes};
use crate::ir::embed::Embed;
use crate::ir::pending::{
    BodyNumberingData, EmbedData, Pending, SidebarAnchorData, SidebarData, SidebarIndexesData,
    SidebarNumberingData,
};
use std::collections::HashMap;

pub struct PendingPass<'a, 'b, 'c> {
    slug: Key,
    global_data: &'c GlobalData<'a, 'b, 'c>,
}

impl<'c, 'b: 'c, 'a: 'b> PendingPass<'a, 'b, 'c> {
    pub fn new(slug: Key, global_data: &'c GlobalData<'a, 'b, 'c>) -> Self {
        Self { slug, global_data }
    }

    pub fn run(
        self,
        content: &'c (Vec<String>, Vec<String>, Vec<String>),
        embeds: &[Embed],
        indexes: &Indexes,
    ) -> Pending<'c> {
        match indexes {
            Indexes::All => self.run_self(embeds.iter().collect(), content),
            Indexes::Some(indexes) => {
                let embeds: Vec<&Embed> = indexes.iter().map(|&i| &embeds[i]).collect();
                self.run_self(embeds, content)
            }
        }
    }

    fn emit_embeds(&self, embeds: Vec<&Embed>) -> Vec<EmbedData<'c>> {
        embeds
            .into_iter()
            .filter_map(|embed| self.emit_embed(embed))
            .collect()
    }

    fn emit_body_numberings(
        &self,
        body_numberings: &HashMap<Pos, usize>,
    ) -> Vec<BodyNumberingData> {
        body_numberings
            .iter()
            .map(|(pos, &body_index)| {
                let pos = pos.clone();
                let anchor = self.slug.to_string();
                BodyNumberingData::new(pos, anchor, body_index)
            })
            .collect()
    }

    fn emit_sidebar_numberings(
        &self,
        sidebar_numberings: &HashMap<Pos, SidebarIndexes>,
    ) -> Vec<SidebarNumberingData> {
        sidebar_numberings
            .iter()
            .map(|(pos, index)| {
                let pos = pos.clone();
                let anchor = self.slug.to_string();
                SidebarNumberingData::new(pos, anchor, index.clone())
            })
            .collect()
    }
    fn emit_sidebar_anchors(
        &self,
        sidebar_anchors: &HashMap<Pos, SidebarIndexes>,
    ) -> Vec<SidebarAnchorData> {
        sidebar_anchors
            .iter()
            .map(|(pos, index)| {
                let pos = pos.clone();
                let anchor = self.slug.to_string();
                SidebarAnchorData::new(pos, anchor, index.clone())
            })
            .collect()
    }
    fn emit_sidebar_indexes(&self, sidebar_show_children: &SidebarIndexes) -> SidebarIndexesData {
        SidebarIndexesData::new(sidebar_show_children.clone())
    }
    fn emit_sidebar(&self, sidebar: &Sidebar) -> SidebarData {
        let indexes = self.emit_sidebar_indexes(sidebar.indexes());
        let numberings = self.emit_sidebar_numberings(sidebar.numberings());
        let anchors = self.emit_sidebar_anchors(sidebar.anchors());
        SidebarData::new(indexes, numberings, anchors)
    }

    fn emit_embed(&self, embed: &Embed) -> Option<EmbedData<'c>> {
        let slug = embed.slug.clone();
        let child = self.global_data.article(slug.as_ref());
        if child.is_none() {
            eprintln!(
                "[WARN] (emit_embed) Embed `{}` not found in {}",
                slug.as_ref(),
                self.slug
            );
            return None;
        }
        let child = child.unwrap();
        let child_metadata = child.get_metadata();
        let child_pending = child.get_pending_or_init(self.global_data);
        let section_type = embed.section_type;
        let pos: Pos = embed.sidebar_pos.0.clone();
        let body_index = embed.body_index;
        let full_sidebar_indexes = embed.full_sidebar_indexes.clone();
        let embed_sidebar_indexes = embed.embed_sidebar_indexes.clone();
        let open = embed.open;
        let variables = embed.variables.clone();
        let title = child_metadata.inline(&self.global_data.config.embed.embed_title.body);
        let full_sidebar_title_indexes = child.get_full_sidebar().title_index().clone();
        let embed_sidebar_title_indexes = child.get_embed_sidebar().title_index().clone();
        Some(EmbedData::new(
            pos,
            slug,
            section_type,
            body_index,
            full_sidebar_indexes,
            embed_sidebar_indexes,
            open,
            variables,
            title,
            full_sidebar_title_indexes,
            embed_sidebar_title_indexes,
            child_pending,
        ))
    }

    fn run_self(
        self,
        embeds: Vec<&Embed>,
        content: &'c (Vec<String>, Vec<String>, Vec<String>),
    ) -> Pending<'c> {
        let article = self.global_data.article(self.slug.as_ref()).unwrap();
        let style = article.get_meta_options().heading_numbering_style;
        let body_numberings = self.emit_body_numberings(&article.get_body().numberings);
        let full_sidebar = article.get_full_sidebar();
        let embed_sidebar = article.get_embed_sidebar();
        let full_sidebar_data = self.emit_sidebar(full_sidebar);
        let embed_sidebar_data = self.emit_sidebar(embed_sidebar);
        let embeds = self.emit_embeds(embeds);
        let anchors = article.get_anchors();
        Pending::new(
            content,
            style,
            body_numberings,
            full_sidebar_data,
            embed_sidebar_data,
            embeds,
            anchors,
        )
    }
}
