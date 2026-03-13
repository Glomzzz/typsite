use crate::compile::error::{TypError, TypResult};
use crate::compile::registry::Key;
use crate::compile::{compile_options, proj_options};
use crate::config::TypsiteConfig;
use crate::ir::article::data::GlobalData;
use crate::ir::article::dep::Indexes;
use crate::ir::rewriter::{MetaRewriter, PureRewriter};
use crate::pass::pass_rewriter_meta;
use crate::util::str::ac_replace_map;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

pub const TITLE_KEY: &str = "title";
pub const TITLE_REPLACEMENT: &str = "{title}";
pub const PAGE_TITLE_REPLACEMENT: &str = "{page-title}";
pub const PAGE_TITLE_REPLACEMENT_: &str = "{page_title}";
pub const SLUG_REPLACEMENT: &str = "{slug}";
pub const SLUG_ANCHOR_REPLACEMENT: &str = "{slug@anchor}";
pub const SLUG_DIPLAY_REPLACEMENT: &str = "{slug-display}";
pub const SLUG_DIPLAY_REPLACEMENT_: &str = "{slug_display}";
pub const HAS_PARENT_REPLACEMENT: &str = "{has-parent}";
pub const HAS_PARENT_REPLACEMENT_: &str = "{has_parent}";

#[derive(Debug, Clone, PartialEq)]
pub struct MetaContents<'a> {
    slug: Key,
    // Content supported
    pub contents: HashMap<String, MetaContent<'a>>,
    replacement: OnceLock<HashMap<String, String>>,
    parent: OnceLock<bool>,
    parent_replacement: OnceLock<HashMap<String, String>>,
    updated: bool, // if the article is updated currently
}

impl<'b, 'a: 'b> MetaContents<'a> {
    pub fn new(
        slug: Key,
        contents: HashMap<String, MetaContent<'a>>,
        updated: bool,
    ) -> MetaContents<'a> {
        MetaContents {
            slug,
            contents,
            replacement: OnceLock::new(),
            parent: OnceLock::new(),
            parent_replacement: OnceLock::new(),
            updated,
        }
    }

    pub fn from(
        self_slug: Key,
        pure: PureMetaContents,
        config: &'a TypsiteConfig,
    ) -> TypResult<MetaContents<'a>> {
        let mut err = TypError::new(self_slug.clone());
        let contents = pure
            .contents
            .into_iter()
            .filter_map(|(meta_key, content)| {
                let result = MetaContent::from(&self_slug, &meta_key, content, config)
                    .map(|content| (meta_key, content));
                err.ok_typ(result)
            })
            .collect();
        err.err_or(|| MetaContents::new(self_slug, contents, false))
    }
    pub fn same_contents(&self, other: &HashMap<String, MetaContent<'a>>) -> bool {
        self.contents.eq(other)
    }
    pub fn is_updated(&self) -> bool {
        self.updated
    }

    pub fn get(&self, key: &str) -> Option<Arc<str>> {
        let content = self.contents.get(key).map(|c| c.get());
        content.or_else(|| {
            proj_options()
                .unwrap()
                .default_metadata
                .content
                .default
                .get(key)
                .cloned()
        })
    }

    pub(crate) fn keys(&self) -> HashSet<&str> {
        self.contents.keys().map(|k| k.as_str()).collect()
    }

    fn init_replacement(&self) -> &HashMap<String, String> {
        self.replacement.get_or_init(|| {
            let mut map = self
                .contents
                .iter()
                .map(|(k, v)| (format!("{{{k}}}"), v.get().to_string()))
                .collect::<HashMap<_, _>>();
            let compile_options = compile_options().unwrap();
            // Short slug
            let slug_display = if compile_options.short_slug {
                self.slug
                    .split('/')
                    .next_back()
                    .unwrap_or(self.slug.as_ref())
            } else {
                self.slug.as_ref()
            };

            map.insert(
                SLUG_DIPLAY_REPLACEMENT.to_string(),
                slug_display.to_string(),
            );
            map.insert(
                SLUG_DIPLAY_REPLACEMENT_.to_string(),
                slug_display.to_string(),
            );

            let slug = if !compile_options.pretty_url {
                format!("{}.html", self.slug)
            } else {
                self.slug.to_string()
            };

            map.insert(SLUG_REPLACEMENT.to_string(), slug.to_string());

            map.insert(SLUG_ANCHOR_REPLACEMENT.to_string(), slug[1..].to_string());

            let parent = self.parent.get().cloned().unwrap_or(false);
            map.insert(HAS_PARENT_REPLACEMENT.to_string(), parent.to_string());
            map.insert(HAS_PARENT_REPLACEMENT_.to_string(), parent.to_string());

            // Add default meta contents
            proj_options()
                .unwrap()
                .default_metadata
                .content
                .default
                .iter()
                .for_each(|(k, v)| {
                    map.entry(format!("{{{k}}}")).or_insert(v.to_string());
                });

            if !map.contains_key(PAGE_TITLE_REPLACEMENT) {
                map.insert(
                    PAGE_TITLE_REPLACEMENT.to_string(),
                    map.get(TITLE_REPLACEMENT)
                        .map(|it| it.to_string())
                        .unwrap_or("Untitled".to_string()),
                );
            }
            if !map.contains_key(PAGE_TITLE_REPLACEMENT_) {
                map.insert(
                    PAGE_TITLE_REPLACEMENT_.to_string(),
                    map.get(TITLE_REPLACEMENT)
                        .map(|it| it.to_string())
                        .unwrap_or("Untitled".to_string()),
                );
            }
            map
        })
    }
    fn replacement(&self) -> Vec<(&str, &str)> {
        let parent_replacement = self.parent_replacement.get();
        if let Some(parent_replacement) = parent_replacement {
            self.init_replacement()
                .iter()
                .chain(parent_replacement.iter())
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect()
        } else {
            self.init_replacement()
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect()
        }
    }

    pub fn init_parent<'c>(&self, global_data: &'c GlobalData<'a, 'b, 'c>) {
        let default_parent_slug = proj_options().ok().and_then(|options| {
            options
                .default_metadata
                .graph
                .default_parent_slug(global_data.config, |slug| {
                    global_data.article(&slug).map(|it| it.slug.clone())
                })
        });

        let self_metadata = global_data.metadata(self.slug.as_ref()).unwrap();

        let parent = self.parent.get_or_init(|| {
            self_metadata.node.parent.is_some()
                || default_parent_slug
                    .as_ref()
                    .map(|default| default.as_ref() != self.slug.as_ref())
                    .unwrap_or(false)
        });
        if !parent {
            return;
        }
        self.parent_replacement.get_or_init(|| {
            self_metadata
                .node
                .parent
                .clone()
                .or_else(|| {
                    if self.parent.get().cloned().unwrap_or(false) {
                        default_parent_slug
                    } else {
                        None
                    }
                })
                .as_ref()
                .and_then(|parent| {
                    global_data
                        .metadata(parent.as_ref())
                        .map(|parent_metadata| {
                            parent_metadata.contents.init_parent(global_data);
                            parent_metadata
                                .contents
                                .init_replacement()
                                .iter()
                                .map(|(k, v)| {
                                    let key = &k[0..k.len() - 1];
                                    let key = format!("{key}@parent}}");
                                    (key, v.to_string())
                                })
                                .collect::<HashMap<_, _>>()
                        })
                })
                .unwrap_or_default()
        });
    }

    pub fn inline_with(&self, text: &str, replacements: &[(&str, &str)]) -> String {
        ac_replace_map(
            text,
            (*self.replacement())
                .iter()
                .chain(replacements.iter())
                .cloned()
                .unzip(),
        )
    }

    pub fn pass_content<'c>(
        &self,
        key: &str,
        indexes: Indexes,
        global_data: &'c GlobalData<'a, 'b, 'c>,
    ) {
        if let Some(content) = self.contents.get(key) {
            content.pass_body(self.slug.clone(), indexes, global_data);
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct MetaContent<'a> {
    content: Vec<String>,
    rewriters: Vec<MetaRewriter<'a>>,
    content_cache: OnceLock<Vec<String>>,
    content_str: OnceLock<Arc<str>>,
}

impl<'b, 'a: 'b> MetaContent<'a> {
    pub fn new(content: Vec<String>, rewriters: Vec<MetaRewriter<'a>>) -> MetaContent<'a> {
        Self {
            content,
            rewriters,
            content_cache: OnceLock::new(),
            content_str: OnceLock::new(),
        }
    }

    fn from(
        self_slug: &Key,
        meta_key: &str,
        pure: PureMetaContent,
        config: &'a TypsiteConfig,
    ) -> TypResult<MetaContent<'a>> {
        let mut err = TypError::new(self_slug.clone());
        let rewriters = pure
            .rewriters
            .into_iter()
            .map(|rewriter| err.ok(MetaRewriter::from(self_slug, meta_key, rewriter, config)))
            .collect::<Vec<Option<_>>>();
        err.err_or(move || {
            let rewriters = rewriters.into_iter().flatten().collect();
            Self::new(pure.body, rewriters)
        })
    }

    fn pass_body<'c>(
        &self,
        slug: Key,
        indexes: Indexes,
        global_data: &'c GlobalData<'a, 'b, 'c>,
    ) -> &Vec<String> {
        self.content_cache.get_or_init(|| {
            let mut body = self.content.clone();
            pass_rewriter_meta(slug, &mut body, &self.rewriters, &indexes, global_data);
            body
        })
    }

    fn get(&self) -> Arc<str> {
        self.content_str
            .get_or_init(|| {
                let str = if let Some(body) = self.content_cache.get() {
                    body.join("")
                } else {
                    self.content.join("")
                };
                Arc::from(str)
            })
            .clone()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PureMetaContents {
    pub contents: HashMap<String, PureMetaContent>,
}

impl From<MetaContents<'_>> for PureMetaContents {
    fn from(content: MetaContents<'_>) -> PureMetaContents {
        let contents: HashMap<String, PureMetaContent> = content
            .contents
            .into_iter()
            .map(|(k, v)| (k.to_string(), PureMetaContent::from(v)))
            .collect();
        PureMetaContents { contents }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct PureMetaContent {
    pub body: Vec<String>,
    pub rewriters: Vec<PureRewriter>,
}

impl PureMetaContent {
    pub fn new(body: Vec<String>, rewriters: Vec<PureRewriter>) -> Self {
        Self { body, rewriters }
    }
}

impl From<MetaContent<'_>> for PureMetaContent {
    fn from(content: MetaContent<'_>) -> Self {
        Self::new(
            content
                .content_cache
                .into_inner()
                .unwrap_or(content.content),
            content
                .rewriters
                .into_iter()
                .map(PureRewriter::from)
                .collect(),
        )
    }
}

impl Serialize for PureMetaContent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Content", 2)?;
        state.serialize_field("body", &self.body)?;
        state.serialize_field("rewriters", &self.rewriters)?;
        state.end()
    }
}

impl<'ce> Deserialize<'ce> for PureMetaContent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'ce>,
    {
        #[derive(Deserialize)]
        struct Temporary {
            body: Vec<String>,
            rewriters: Vec<PureRewriter>,
        }
        let temp = Temporary::deserialize(deserializer)?;
        Ok(PureMetaContent::new(temp.body, temp.rewriters))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::metadata::content::PureMetaContent;
    use crate::ir::rewriter::{PureRewriter, RewriterType};
    use std::collections::HashMap;

    #[test]
    fn cont_serialize_and_de() {
        let content = PureMetaContent::new(
            vec!["Hello".into(), "World".into()],
            vec![PureRewriter::new(
                "test".into(),
                RewriterType::Start,
                HashMap::new(),
                vec![1, 2, 3].into_iter().collect(),
                114514,
            )],
        );

        let json = serde_json::to_string(&content).unwrap();
        let decoded: PureMetaContent = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, content);
    }

    fn plain(plain: &str) -> PureMetaContent {
        PureMetaContent::new(vec![plain.to_string()], vec![])
    }
    #[test]
    fn metadata_serialize_and_de() {
        // let slug = "test/test".to_string();
        let contents = [
            ("title".to_string(), plain("Test")),
            ("taxon".to_string(), plain("test")),
            ("page-title".to_string(), plain("Test")),
            ("date".to_string(), plain("2024-10-20")),
            ("author".to_string(), plain("Glom")),
        ]
        .into_iter();
        let content = PureMetaContents {
            contents: contents.collect(),
        };
        let json = serde_json::to_string(&content).unwrap();
        let metadata_de = serde_json::from_str(&json).unwrap();
        assert_eq!(content, metadata_de)
    }
}
