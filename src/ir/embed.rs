use crate::compile::registry::{Key, KeyRegistry};
use crate::ir::article::sidebar::{SidebarIndexes, SidebarPos};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};

pub type EmbedVariables = Vec<(String, String)>;

pub struct Embed {
    pub slug: Key,
    pub open: bool,
    pub variables: EmbedVariables,
    pub section_type: SectionType,
    pub sidebar_pos: SidebarPos,
    pub full_sidebar_indexes: SidebarIndexes,
    pub embed_sidebar_indexes: SidebarIndexes,
    pub body_index: usize,
}

impl Embed {
    pub fn new(
        slug: Key,
        open: bool,
        variables: EmbedVariables,
        section_type: SectionType,
        sidebar_pos: SidebarPos,
        full_sidebar_indexes: SidebarIndexes,
        embed_sidebar_indexes: SidebarIndexes,
        body_index: usize,
    ) -> Self {
        Embed {
            slug,
            open,
            variables,
            section_type,
            sidebar_pos,
            full_sidebar_indexes,
            embed_sidebar_indexes,
            body_index,
        }
    }

    pub fn from(self_slug: &str, pure: PureEmbed, registry: &KeyRegistry) -> anyhow::Result<Embed> {
        let slug = pure.slug;
        match registry.slug(slug.as_str()) {
            Some(slug) => Ok(Embed::new(
                slug,
                pure.open,
                pure.variables,
                pure.section_type,
                pure.sidebar_pos,
                pure.full_sidebar_indexes,
                pure.embed_sidebar_indexes,
                pure.body_index,
            )),
            None => Err(anyhow!(
                "Embed article not found: {slug} in {self_slug}, skip this embedding."
            )),
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct PureEmbed {
    slug: String,
    open: bool,
    variables: EmbedVariables,
    section_type: SectionType,
    sidebar_pos: SidebarPos,
    full_sidebar_indexes: SidebarIndexes,
    embed_sidebar_indexes: SidebarIndexes,
    body_index: usize,
}

impl From<Embed> for PureEmbed {
    fn from(embed: Embed) -> Self {
        PureEmbed {
            slug: embed.slug.to_string(),
            open: embed.open,
            variables: embed.variables,
            section_type: embed.section_type,
            sidebar_pos: embed.sidebar_pos,
            full_sidebar_indexes: embed.full_sidebar_indexes,
            embed_sidebar_indexes: embed.embed_sidebar_indexes,
            body_index: embed.body_index,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Copy, Clone)]
pub enum SectionType {
    None,
    OnlyTitle,
    Full,
}

impl<T: AsRef<str>> From<T> for SectionType {
    fn from(s: T) -> Self {
        match s.as_ref() {
            "none" => SectionType::None,
            "only_title" | "only-title" => SectionType::OnlyTitle,
            "full" => SectionType::Full,
            _ => {
                eprintln!("[WARN] Invalid sidebar type: {}", s.as_ref());
                SectionType::Full
            }
        }
    }
}

// PureAtom::Embed { slug, open, sidebar_pos, index } => match registry.know(slug,"Embed",self_slug) {
// Some(slug) => Atom::Embed { slug, open, sidebar_pos, index },
// None =>Atom::Plain
// }

// #[serde(rename = "embed")]
// Embed {
// slug: String,
// open: bool,
// #[serde(default, skip_serializing_if = "Option::is_none", with = "pos_format")]
// sidebar_pos: Option<(Vec<usize>,usize)>,
// index: usize
// },
