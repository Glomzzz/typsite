use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub struct CompileOptions {
    pub watch: bool,
    pub short_slug: bool,
    pub pretty_url: bool,
}

pub const OPTIONS_PATH: &str = "options.toml";
#[derive(Debug, Deserialize)]
pub struct ProjOptions {
    pub default_metadata: DefaultMetadata,
    #[serde(deserialize_with = "lib_paths::deserialize_lib_paths")]
    pub typst_lib: TypstLib,
    pub code_fallback_style: CodeFallbackStyle,
    #[serde(default)]
    pub site: SiteOptions,
}

#[derive(Debug, Deserialize)]
pub struct TypstLib {
    pub files: HashSet<String>,
    pub dirs: HashSet<String>,
}
#[derive(Debug, Deserialize)]
pub struct DefaultMetadata {
    pub content: metadata::Content,
    pub options: metadata::Options,
    pub graph: metadata::Graph,
}

impl ProjOptions {
    pub fn load(config_path: &Path) -> anyhow::Result<Self> {
        let content = fs::read_to_string(config_path.join(OPTIONS_PATH))?;
        let options: Self = toml::from_str(&content)?;
        Ok(options)
    }
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CodeFallbackStyle {
    pub dark: String,
    pub light: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SiteOptions {
    #[serde(default = "default_site_base_url")]
    pub base_url: String,
    #[serde(default = "default_site_title")]
    pub title: String,
    #[serde(default = "default_site_description")]
    pub description: String,
    #[serde(default = "default_rss_path")]
    pub rss_path: String,
    #[serde(default = "default_sitemap_path")]
    pub sitemap_path: String,
    #[serde(default = "default_rss_limit")]
    pub rss_limit: usize,
}

fn default_site_base_url() -> String {
    String::new()
}

fn default_site_title() -> String {
    String::new()
}

fn default_site_description() -> String {
    String::new()
}

fn default_rss_path() -> String {
    "/rss.xml".to_string()
}

fn default_sitemap_path() -> String {
    "/sitemap.xml".to_string()
}

fn default_rss_limit() -> usize {
    20
}
pub mod metadata {
    use crate::{
        compile::{proj_options, registry::Key},
        config::TypsiteConfig,
        ir::article::sidebar::{HeadingNumberingStyle, SidebarType},
    };
    use serde::{Deserialize, Serialize};
    use std::{
        collections::HashMap,
        sync::{Arc, OnceLock},
    };

    #[derive(Debug, Deserialize)]
    pub struct Content {
        #[serde(
            flatten,
            default = "HashMap::new",
            deserialize_with = "deserialize_meta_content"
        )]
        pub default: HashMap<String, Arc<str>>,
    }
    #[derive(Debug, Deserialize, Serialize)]
    pub struct Options {
        #[serde(default = "default_heading_numbering")]
        pub heading_numbering: HeadingNumberingStyle,
        #[serde(default = "default_sidebar_type")]
        pub sidebar_type: SidebarType,
    }
    #[derive(Debug, Deserialize, Serialize)]
    pub struct Graph {
        #[serde(
            default = "default_parent",
            deserialize_with = "deserialize_optional_parent"
        )]
        pub parent: Option<String>,
        #[serde(skip)]
        default_parent_slug: OnceLock<Option<Key>>,
    }
    impl Graph {
        pub fn default_parent_slug(
            &self,
            config: &TypsiteConfig,
            verify_slug: impl FnOnce(String) -> Option<Key>,
        ) -> Option<Key> {
            self.default_parent_slug
                .get_or_init(|| {
                    proj_options().ok().and_then(|it| {
                        it.default_metadata
                            .graph
                            .parent
                            .as_ref()
                            .map(|slug| config.format_slug(slug))
                            .and_then(verify_slug)
                    })
                })
                .clone()
        }
    }

    pub fn default_sidebar_type() -> SidebarType {
        SidebarType::All
    }
    pub fn default_heading_numbering() -> HeadingNumberingStyle {
        HeadingNumberingStyle::Bullet
    }
    pub fn default_parent() -> Option<String> {
        None
    }

    pub fn deserialize_optional_parent<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        Ok(if s.trim().is_empty() { None } else { Some(s) })
    }
    pub fn deserialize_meta_content<'de, D>(
        deserializer: D,
    ) -> Result<HashMap<String, Arc<str>>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: HashMap<String, String> = Deserialize::deserialize(deserializer)?;
        let map = s
            .into_iter()
            .map(|(key, value)| (key, Arc::from(value)))
            .collect::<HashMap<String, Arc<str>>>();
        Ok(map)
    }
}

mod lib_paths {

    use std::collections::HashSet;

    use serde::{Deserialize, Deserializer};

    use super::TypstLib;

    #[derive(Deserialize)]
    struct RawTypstLib {
        paths: Vec<String>,
    }

    pub fn deserialize_lib_paths<'de, D>(deserializer: D) -> Result<TypstLib, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawTypstLib::deserialize(deserializer)?;
        let mut files = HashSet::new();
        let mut dirs = HashSet::new();

        for path in raw.paths {
            if path.to_lowercase().ends_with("/") {
                dirs.insert(path);
            } else {
                files.insert(path);
            }
        }

        Ok(TypstLib { files, dirs })
    }
}
