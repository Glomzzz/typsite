use crate::config::SCHEMAS_DIR;
use crate::util::error::log_err_or_ok;
use crate::util::html::Html;
use crate::util::path::file_stem;
use crate::walk_glob;
use anyhow::*;
use glob::glob;
use html5gum::Token;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::TypsiteConfig;

pub const BACKLINK_KEY: &str = "backlink";
pub const REFERENCE_KEY: &str = "reference";

pub struct SchemaConfig {
    pub schemas: HashMap<String, Schema>,
}

impl SchemaConfig {
    pub fn load(config_path: &Path) -> Result<Self> {
        let schemas_path = config_path.join(SCHEMAS_DIR);
        let mut schemas = HashMap::new();
        let loaded_schemas = walk_glob!("{}/**/*.html", schemas_path.display())
            .par_bridge()
            .filter_map(|path| Some((file_stem(&path)?.to_string(), path)))
            .map(Schema::load_schema)
            .collect::<Vec<Result<(String, Schema)>>>();
        schemas.extend(loaded_schemas.into_iter().filter_map(log_err_or_ok));
        Ok(Self { schemas })
    }

    pub fn get(&self, id: &str) -> Result<&Schema> {
        self.schemas
            .get(id)
            .with_context(|| format!("No schema named {id}"))
    }
}

const PARENT_KEYWORD: &str = "@parent}";

/**
* A struct that represents a Schema.
*/
pub struct Schema {
    pub id: String,
    pub path: Arc<Path>,
    pub content: bool,
    pub sidebar: bool,
    pub footer: bool,
    pub parent: bool, // If used parent's metacontent by {key@parent}
    pub head: String,
    pub body: String,
}
impl Schema {
    fn load_schema((id, path): (String, PathBuf)) -> Result<(String, Self)> {
        let mut content = false;
        let mut sidebar = false;
        let mut footer = false;
        let Html { head, body } = Html::load_with_body_callback(&path, |token| {
            // let chains
            if let Token::StartTag(tag) = token {
                match tag.name.as_slice() {
                    b"content" => {
                        content = true;
                    }
                    b"sidebar" => {
                        sidebar = true;
                    }
                    b"footer" => {
                        footer = true;
                    }
                    _ => {}
                }
            }
            Ok(())
        })?;
        let parent = head.contains(PARENT_KEYWORD) || body.contains(PARENT_KEYWORD);
        let path = Arc::from(path);
        let schema = Self {
            id,
            path,
            content,
            sidebar,
            footer,
            parent,
            head,
            body,
        };
        Ok((schema.id.clone(), schema))
    }
    pub fn component_paths(&self, config: &TypsiteConfig<'_>) -> HashSet<Arc<Path>> {
        let mut files: HashSet<Arc<Path>> = HashSet::new();
        files.insert(self.path.clone());
        if self.content {
            files.insert(config.section.path.clone());
            files.insert(config.heading_numbering.path.clone());
        }
        if self.sidebar {
            files.insert(config.sidebar.each_path.clone());
            files.insert(config.sidebar.block_path.clone());
        }
        if self.footer {
            files.insert(config.footer.footer.path.clone());
            files.insert(config.footer.backlinks.path.clone());
            files.insert(config.footer.references.path.clone());
            let _ = config
                .schemas
                .get(BACKLINK_KEY)
                .map(|schema| files.insert(schema.path.clone()));
            let _ = config
                .schemas
                .get(REFERENCE_KEY)
                .map(|schema| files.insert(schema.path.clone()));
        }
        files
    }
}
