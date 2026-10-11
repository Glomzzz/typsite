use crate::config::{FOOTER_BACKLINKS_PATH, FOOTER_PATH, FOOTER_REFERENCES_PATH, HtmlConfig};
use std::path::Path;

pub const REFERENCES_KEY: &str = "{references}";
pub const BACKLINKS_KEY: &str = "{backlinks}";

pub struct FooterConfig {
    pub footer: HtmlConfig,
    pub backlinks: HtmlConfig,
    pub references: HtmlConfig,
}

impl FooterConfig {
    pub fn load(config: &Path) -> anyhow::Result<Self> {
        let footer = HtmlConfig::load(config, FOOTER_PATH)?;
        let backlinks = HtmlConfig::load(config, FOOTER_BACKLINKS_PATH)?;
        let references = HtmlConfig::load(config, FOOTER_REFERENCES_PATH)?;
        Ok(Self {
            footer,
            backlinks,
            references,
        })
    }
}
