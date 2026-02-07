use crate::compile::registry::Key;
use crate::config::TypsiteConfig;
use crate::config::anchor::AnchorConfig;
use crate::config::heading_numbering::HeadingNumberingConfig;
use crate::ir::article::data::GlobalData;
use crate::ir::article::sidebar::{HeadingNumberingStyle, Pos, SidebarIndexes, SidebarType};
use crate::ir::embed::SectionType;
use crate::util::str::{SectionElem, ac_replace};
use crate::util::{pos_base_on, pos_slug};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::embed::EmbedVariables;

pub struct BodyNumberingData {
    pos: Pos,
    anchor: String,
    body_index: usize,
}

impl BodyNumberingData {
    pub fn new(pos: Pos, anchor: String, body_index: usize) -> Self {
        Self {
            pos,
            anchor,
            body_index,
        }
    }
    fn based_on(
        &self,
        config: &HeadingNumberingConfig,
        base_anchor: Option<&Pos>,
        base_numbering: Option<&Pos>,
        style: HeadingNumberingStyle,
        body: &mut [String],
    ) {
        let numbering = config.get_with_pos_anchor(
            style,
            base_anchor,
            base_numbering,
            &self.pos,
            self.anchor.as_str(),
        );
        body[self.body_index] = numbering.clone();
    }
}

pub struct SidebarData {
    indexes: SidebarIndexesData,
    numberings: Vec<SidebarNumberingData>,
    anchors: Vec<SidebarAnchorData>,
}

impl SidebarData {
    fn based_on(
        &self,
        config: &HeadingNumberingConfig,
        base_anchor: Option<&Pos>,
        base_numbering: Option<&Pos>,
        section_type: SectionType,
        style: HeadingNumberingStyle,
        sidebar: &mut [String],
    ) {
        self.indexes.based_on(section_type, sidebar);
        for numbering in self.numberings.iter() {
            numbering.based_on(config, base_anchor, base_numbering, style, sidebar);
        }
        for anchor in self.anchors.iter() {
            anchor.based_on(base_anchor, sidebar);
        }
    }

    pub fn new(
        indexes: SidebarIndexesData,
        numberings: Vec<SidebarNumberingData>,
        anchors: Vec<SidebarAnchorData>,
    ) -> Self {
        Self {
            indexes,
            numberings,
            anchors,
        }
    }
}

pub struct SidebarNumberingData {
    pos: Pos,
    anchor: String,
    sidebar_indexes: SidebarIndexes,
}
impl SidebarNumberingData {
    pub fn new(pos: Pos, anchor: String, sidebar_indexes: SidebarIndexes) -> Self {
        Self {
            pos,
            anchor,
            sidebar_indexes,
        }
    }
    fn based_on(
        &self,
        config: &HeadingNumberingConfig,
        base_anchor: Option<&Pos>,
        base_numbering: Option<&Pos>,
        style: HeadingNumberingStyle,
        sidebar: &mut [String],
    ) {
        let numbering = config.get_with_pos_anchor(style, base_anchor,base_numbering, &self.pos, self.anchor.as_str());
        for &index in &self.sidebar_indexes {
            sidebar[index] = numbering.clone();
        }
    }
}
pub struct SidebarAnchorData {
    pos: Pos,
    anchor: String,
    sidebar_indexes: SidebarIndexes,
}
impl SidebarAnchorData {
    pub fn new(pos: Pos, anchor: String, sidebar_indexes: SidebarIndexes) -> Self {
        Self {
            pos,
            anchor,
            sidebar_indexes,
        }
    }
    fn based_on(
        &self,
        base_anchor: Option<&Pos>,
        sidebar: &mut [String],
    ) {
        let pos_anchor = pos_base_on(base_anchor, Some(&self.pos));
        let anchor = pos_slug(&pos_anchor, &self.anchor);
        for &index in &self.sidebar_indexes {
            sidebar[index] = anchor.clone();
        }
    }
}
pub struct SidebarIndexesData {
    sidebar_indexes: SidebarIndexes,
}
impl SidebarIndexesData {
    pub fn new(sidebar_indexes: SidebarIndexes) -> Self {
        Self { sidebar_indexes }
    }
    fn based_on(&self, section_type: SectionType, sidebar: &mut [String]) {
        let if_show = match section_type {
            SectionType::OnlyTitle => "none",
            _ => "block",
        };
        for &index in &self.sidebar_indexes {
            sidebar[index] = if_show.to_string();
        }
    }
}

pub struct EmbedData<'c> {
    pos: Pos,
    slug: Key,
    body_index: usize,
    full_sidebar_indexes: SidebarIndexes,
    embed_sidebar_indexes: SidebarIndexes,
    open: bool,
    variables: EmbedVariables,
    title: String,
    full_sidebar_title_indexes: SidebarIndexes,
    embed_sidebar_title_indexes: SidebarIndexes,
    pending: &'c Pending<'c>,
    pub section_type: SectionType,
}

impl<'c> EmbedData<'c> {
    pub fn new(
        pos: Pos,
        slug: Key,
        section_type: SectionType,
        body_index: usize,
        full_sidebar_indexes: SidebarIndexes,
        embed_sidebar_indexes: SidebarIndexes,
        open: bool,
        variables: EmbedVariables,
        title: String,
        full_sidebar_title_indexes: SidebarIndexes,
        embed_sidebar_title_indexes: SidebarIndexes,
        child: &'c Pending,
    ) -> Self {
        Self {
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
            pending: child,
        }
    }

    fn based_on(
        &self,
        config: &TypsiteConfig,
        global_data: &'c GlobalData<'_, '_, 'c>,
        base_anchor: Option<&Pos>,
        base_numbering: Option<&Pos>,
        parent_style: HeadingNumberingStyle,
        body: &mut [String],
        sidebar: &mut [String],
        sidebar_type: SidebarType,
    ) {
        let pos = &self.pos;
        let metadata = global_data.metadata(self.slug.as_str()).unwrap();
        let numbering = config.heading_numbering.get_with_pos_anchor(
            parent_style,
            base_anchor,
            base_numbering,
            pos,
            &self.slug,
        ); // also as anchor

        let mut embed_article_body = Vec::new();
        let mut embed_article_sidebar = String::new();

        let embed_config = &config.embed.embed;

        let pos_anchor = Some(pos);
        let pos_numebring = match parent_style {
            HeadingNumberingStyle::None => None,
            _ => Some(pos),
        };

        let base_anchor = Some(pos_base_on(base_anchor, pos_anchor));
        let base_numbering = Some(pos_base_on(base_numbering, pos_numebring));

        let (body_vec, mut full_sidebar_vec, mut embed_sidebar_vec) = self.pending.based_on(
            config,
            global_data,
            base_anchor.as_ref(),
            base_numbering.as_ref(),
            sidebar_type,
            self.section_type,
        );

        if sidebar_type == SidebarType::All {
            match self.section_type {
                SectionType::None => {}
                _ => {
                    for &title_index in &self.full_sidebar_title_indexes {
                        full_sidebar_vec[title_index] = self.title.clone();
                    }
                    embed_article_sidebar = full_sidebar_vec.join("");
                }
            }
        } else {
            for &title_index in &self.embed_sidebar_title_indexes {
                embed_sidebar_vec[title_index] = self.title.clone();
            }
            embed_article_sidebar = embed_sidebar_vec.join("")
        };

        for elem in &embed_config.body {
            let str = match elem {
                SectionElem::Plain(plain) => plain.clone(),
                SectionElem::Content => body_vec.join(""),
                SectionElem::Level => self.slug.to_string(),
                SectionElem::Title => self.title.clone(),
                SectionElem::HeadingNumbering => numbering.clone(),
            };
            embed_article_body.push(str);
        }
        let mut replacements: Vec<(&str, &str)> = self
            .variables
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        replacements.push(("{open}=\"\"", if self.open { "open=\"\"" } else { "" }));

        let embed_body = embed_article_body.join("");
        let embed_body = ac_replace(&embed_body, &replacements);
        let embed_body = metadata.inline(&embed_body);
        body[self.body_index] = embed_body;
        let indexes = if sidebar_type == SidebarType::All {
            &self.full_sidebar_indexes
        } else {
            &self.embed_sidebar_indexes
        };
        for &index in indexes {
            sidebar[index] = embed_article_sidebar.clone();
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum AnchorKind {
    Define,
    GotoHead,
    GotoTail,
    DefineSvg{transform: String},
    GotoHeadSvg{transform: String},
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnchorData {
    anchor: String,
    kind: AnchorKind,
    body_indexes: HashSet<usize>,
}

impl AnchorData {
    pub fn new(anchor: String, kind: AnchorKind,  body_indexes: HashSet<usize>) -> Self {
        Self {
            anchor,
            kind,
            body_indexes,
        }
    }
    fn based_on(&self, config: &AnchorConfig, base: Option<&Pos>, body: &mut [String]) {
        let text = config.get(&self.kind, base, &self.anchor);
        for &index in &self.body_indexes {
            body[index] = text.clone();
        }
    }
}

pub struct Pending<'c> {
    // body, sidebar
    pub raw: &'c (Vec<String>, Vec<String>, Vec<String>),
    pub style: HeadingNumberingStyle,
    pub body_numberings: Vec<BodyNumberingData>,
    pub full_sidebar_data: SidebarData,
    pub embed_sidebar_data: SidebarData,
    pub embeds: Vec<EmbedData<'c>>,
    pub anchors: &'c Vec<AnchorData>,
}

impl<'c> Pending<'c> {
    pub fn new(
        raw: &'c (Vec<String>, Vec<String>, Vec<String>),
        style: HeadingNumberingStyle,
        body_numberings: Vec<BodyNumberingData>,
        full_sidebar_data: SidebarData,
        embed_sidebar_data: SidebarData,
        embeds: Vec<EmbedData<'c>>,
        anchors: &'c Vec<AnchorData>,
    ) -> Self {
        Self {
            raw,
            style,
            body_numberings,
            full_sidebar_data,
            embed_sidebar_data,
            embeds,
            anchors,
        }
    }
    pub fn based_on(
        &self,
        config: &TypsiteConfig,
        global_data: &'c GlobalData<'_, '_, 'c>,
        base_anchor: Option<&Pos>,
        base_numbering: Option<&Pos>,
        sidebar_type: SidebarType,
        section_type: SectionType,
    ) -> (Vec<String>, Vec<String>, Vec<String>) {
        let (mut body, mut full_sidebar, mut embed_sidebar) = self.raw.clone();
        let style = self.style;

        for numbering_in_body in &self.body_numberings {
            numbering_in_body.based_on(
                &config.heading_numbering,
                base_anchor,
                base_numbering,
                style,
                &mut body,
            );
        }
        for anchor in self.anchors {
            anchor.based_on(&config.anchor, base_anchor, &mut body);
        }
        let (data, sidebar) = match sidebar_type {
            SidebarType::All => (&self.full_sidebar_data, &mut full_sidebar),
            SidebarType::OnlyEmbed => (&self.embed_sidebar_data, &mut embed_sidebar),
        };

        data.based_on(
            &config.heading_numbering,
            base_anchor,
            base_numbering,
            section_type,
            style,
            sidebar,
        );

        for embed in &self.embeds {
            embed.based_on(
                config,
                global_data,
                base_anchor,
                base_numbering,
                style,
                &mut body,
                sidebar,
                sidebar_type,
            );
        }

        (body, full_sidebar, embed_sidebar)
    }
}
