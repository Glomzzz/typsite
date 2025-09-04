use crate::config::anchor::AnchorConfig;
use crate::config::footer::FooterConfig;
use crate::config::heading_numbering::HeadingNumberingConfig;
use crate::config::highlight::CodeHightlightConfig;
use crate::config::rewrite::RulesConfig;
use crate::config::schema::SchemaConfig;
use crate::config::section::SectionConfig;
use crate::util::html::Html;
use crate::util::path::{dir_name, file_stem};
use crate::util::path::{file_ext, format_path_ref};
use anyhow::{Context, Result, anyhow};
use embed::EmbedConfig;
use sidebar::SidebarConfig;
use std::path::Path;
use std::sync::Arc;

pub mod anchor;
pub mod embed;
pub mod footer;
pub mod heading_numbering;
pub mod highlight;
pub mod rewrite;
pub mod schema;
pub mod section;
pub mod sidebar;

const RULES_DIR: &str = "rewrite/";
const SCHEMAS_DIR: &str = "schemas/";

const THEMES_DIR: &str = "themes";
const SYNTAXES_DIR: &str = "syntaxes";

const SECTION_PATH: &str = "components/section.html";
const HEADING_NUMBERING_PATH: &str = "components/heading-numbering.html";
const FOOTER_BACKLINKS_PATH: &str = "components/footer/backlinks.html";
const FOOTER_REFERENCES_PATH: &str = "components/footer/references.html";
const ANCHOR_DEF_PATH: &str = "components/anchor_def.html";
const ANCHOR_GOTO_PATH: &str = "components/anchor_goto.html";
const FOOTER_PATH: &str = "components/footer.html";
const EMBED_PATH: &str = "components/embed.html";
const EMBED_TITLE_PATH: &str = "components/embed_title.html";

const SIDEBAR_BLOCK_PATH: &str = "components/sidebar.html";
const SIDEBAR_EACH_PATH: &str = "components/sidebar_each.html";
pub struct TypsiteConfig<'a> {
    pub section: SectionConfig,
    pub heading_numbering: HeadingNumberingConfig,
    pub footer: FooterConfig,
    pub anchor: AnchorConfig,
    pub sidebar: SidebarConfig,
    pub embed: EmbedConfig,
    pub rules: RulesConfig,
    pub schemas: SchemaConfig,
    pub highlight: CodeHightlightConfig,
    config_path: &'a Path,
    pub typst_path: &'a Path,
    pub typst_root_name: String,
    pub html_path: &'a Path,
}

impl<'a> TypsiteConfig<'a> {
    pub fn load(config_path: &'a Path, typst_path: &'a Path, html_path: &'a Path) -> Result<Self> {
        // Components
        let section = SectionConfig::load(config_path)?;
        let heading_numbering = HeadingNumberingConfig::load(config_path)?;
        let footer = FooterConfig::load(config_path)?;
        let anchor = AnchorConfig::load(config_path)?;
        let sidebar = SidebarConfig::load(config_path)?;
        let embed = EmbedConfig::load(config_path)?;
        let rules = RulesConfig::load(config_path)?;
        let schemas = SchemaConfig::load(config_path)?;
        let highlight = CodeHightlightConfig::load(config_path);

        let typst_path = format_path_ref(typst_path);
        let typst_root_name = dir_name(typst_path)
            .context("Failed to load input path")?
            .to_string();

        let config = Self {
            rules,
            section,
            heading_numbering,
            footer,
            anchor,
            sidebar,
            embed,
            schemas,
            highlight,
            config_path,
            typst_path,
            typst_root_name,
            html_path,
        };
        Ok(config)
    }

    pub fn path_ref(&'a self, path: &Path) -> Option<Arc<Path>> {
        if path.starts_with(self.typst_path) {
            None
        } else if path.starts_with(self.config_path) {
            let path = path.strip_prefix(self.config_path).ok()?;
            let path_str = path.as_os_str().to_str()?;
            let path_str = path_str.replace("\\", "/");
            match path_str.as_str() {
                SECTION_PATH => Some(self.section.path.clone()),
                HEADING_NUMBERING_PATH => Some(self.heading_numbering.path.clone()),
                FOOTER_PATH => Some(self.footer.footer.path.clone()),
                FOOTER_BACKLINKS_PATH => Some(self.footer.backlinks.path.clone()),
                FOOTER_REFERENCES_PATH => Some(self.footer.references.path.clone()),
                ANCHOR_DEF_PATH => Some(self.anchor.define.path.clone()),
                ANCHOR_GOTO_PATH => Some(self.anchor.goto.path.clone()),
                SIDEBAR_BLOCK_PATH => Some(self.sidebar.block_path.clone()),
                SIDEBAR_EACH_PATH => Some(self.sidebar.each_path.clone()),
                EMBED_PATH => Some(self.embed.embed_path.clone()),
                EMBED_TITLE_PATH => Some(self.embed.embed_title_path.clone()),
                path_str if path_str.starts_with(RULES_DIR) => file_stem(path)
                    .and_then(|rule| self.rules.get(rule))
                    .and_then(|rule| rule.path.clone()),
                path_str if path_str.starts_with(SCHEMAS_DIR) => {
                    file_stem(path).and_then(|schema| {
                        self.schemas
                            .get(schema)
                            .ok()
                            .map(|schema| schema.path.clone())
                    })
                }
                path_str if path_str.starts_with(THEMES_DIR) => {
                    file_stem(path).and_then(|theme| self.highlight.find_theme_path(theme))
                }
                path_str if path_str.starts_with(SYNTAXES_DIR) => {
                    file_stem(path).and_then(|stem| {
                        file_ext(path).and_then(|it| match it.as_str() {
                            "sublime-syntax" => self.highlight.find_syntax_path_by_stem(stem),
                            "tmPreferences" => self.highlight.find_metadata_path_by_stem(stem),
                            _ => None,
                        })
                    })
                }
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn path_to_slug(&self, path: &Path) -> Result<String> {
        let path = if path.starts_with(self.typst_path) {
            path
        } else if path.starts_with(self.html_path) {
            path.strip_prefix(self.html_path)?
        } else {
            return Err(anyhow!("Can't find a slug for {:?}", path));
        };
        Ok(self.format_slug(path.to_string_lossy().as_ref()))
    }

    pub fn format_slug(&self, slug: &str) -> String {
        let slug = slug.replace("\\", "/");
        let slug = slug.trim_start_matches(self.typst_root_name.as_str());
        let slug = slug
            .strip_suffix(".typ")
            .or_else(|| slug.strip_suffix(".html"))
            .unwrap_or(slug);
        if !slug.starts_with('/') {
            format!("/{slug}")
        } else {
            slug.to_string()
        }
    }
}

pub struct HtmlConfig {
    pub path: Arc<Path>,
    pub head: String,
    pub body: String,
}

impl HtmlConfig {
    pub fn load(config: &Path, path: &str) -> Result<Self> {
        let path = Arc::from(config.join(path));
        let Html { head, body } = Html::load(&path)?;
        Ok(Self { path, head, body })
    }
}
