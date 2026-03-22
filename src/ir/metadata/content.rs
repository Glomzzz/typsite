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
                .expect("project options should be initialized before reading metadata defaults")
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
            let compile_options = compile_options()
                .expect("compile options should be initialized before metadata replacements");
            let (short_slug, pretty_url) = (compile_options.short_slug, compile_options.pretty_url);
            // Short slug
            let slug_display = if short_slug {
                self.slug
                    .rsplit('/')
                    .find(|segment| !segment.is_empty())
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

            let slug = if !pretty_url {
                format!("{}.html", self.slug)
            } else {
                self.slug.to_string()
            };

            map.insert(SLUG_REPLACEMENT.to_string(), slug.to_string());

            let slug_anchor = slug
                .strip_prefix('/')
                .expect("metadata slug anchors should only be built from normalized slugs with a leading '/'");
            map.insert(SLUG_ANCHOR_REPLACEMENT.to_string(), slug_anchor.to_string());

            let parent = *self.parent.get_or_init(|| false);
            map.insert(HAS_PARENT_REPLACEMENT.to_string(), parent.to_string());
            map.insert(HAS_PARENT_REPLACEMENT_.to_string(), parent.to_string());

            // Add default meta contents
            proj_options()
                .expect("project options should be initialized before metadata replacements")
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
        let default_parent_slug = proj_options()
            .expect("project options should be initialized before metadata parent replacements")
            .default_metadata
            .graph
            .default_parent_slug(global_data.config, |slug| {
                global_data.article(&slug).map(|it| it.slug.clone())
            });

        let self_metadata = global_data.metadata(self.slug.as_ref()).expect(
            "metadata parent replacements should only initialize for slugs present in GlobalData",
        );

        let resolved_parent_slug = self_metadata.node.parent.clone().or_else(|| {
            default_parent_slug
                .clone()
                .filter(|default| default.as_ref() != self.slug.as_ref())
        });
        let parent = self.parent.get_or_init(|| resolved_parent_slug.is_some());
        if !parent {
            return;
        }
        self.parent_replacement.get_or_init(|| {
            let parent = resolved_parent_slug
                .clone()
                .expect("metadata parent replacements should resolve a concrete parent slug when `has-parent` is true");

            let parent_metadata = global_data
                .metadata(parent.as_ref())
                .expect("metadata parent replacements should only reference parent slugs present in GlobalData");
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
pub(crate) mod tests {
    use super::*;
    use crate::compile::{
        init_compile_options, init_proj_options,
        options::{CompileOptions, ProjOptions},
    };
    use crate::config::TypsiteConfig;
    use crate::ir::article::body::Body;
    use crate::ir::article::data::GlobalData;
    use crate::ir::article::dep::{Dependency, Indexes};
    use crate::ir::article::{sidebar::Sidebar, Article};
    use crate::ir::metadata::content::PureMetaContent;
    use crate::ir::metadata::graph::MetaNode;
    use crate::ir::metadata::options::MetaOptions;
    use crate::ir::metadata::Metadata;
    use crate::ir::rewriter::{PureRewriter, RewriterType};
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::fs;
    use std::sync::{Arc, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

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

    fn write_test_config(root: &std::path::Path, options: &str) {
        fs::create_dir_all(root.join("components/footer")).expect("footer dir");
        fs::create_dir_all(root.join("rewrite")).expect("rewrite dir");
        fs::create_dir_all(root.join("schemas")).expect("schema dir");
        fs::create_dir_all(root.join("syntaxes")).expect("syntaxes dir");
        fs::create_dir_all(root.join("themes")).expect("themes dir");
        fs::write(root.join("options.toml"), options).expect("options file");
        fs::write(
            root.join("components/section.html"),
            include_str!("../../../resources/.typsite/components/section.html"),
        )
        .expect("section");
        fs::write(
            root.join("components/heading-numbering.html"),
            "<html><body>{numbering}</body></html>",
        )
        .expect("heading numbering");
        fs::write(
            root.join("components/footer.html"),
            "<html><body><footer>{references}{backlinks}</footer></body></html>",
        )
        .expect("footer");
        fs::write(
            root.join("components/footer/backlinks.html"),
            "<html><body>{backlinks}</body></html>",
        )
        .expect("backlinks");
        fs::write(
            root.join("components/footer/references.html"),
            "<html><body>{references}</body></html>",
        )
        .expect("references");
        fs::write(
            root.join("components/anchor_def.html"),
            "<html><body></body></html>",
        )
        .expect("anchor def");
        fs::write(
            root.join("components/anchor_def_svg.html"),
            "<html><body></body></html>",
        )
        .expect("anchor def svg");
        fs::write(
            root.join("components/anchor_goto.html"),
            "<html><body></body></html>",
        )
        .expect("anchor goto");
        fs::write(
            root.join("components/anchor_goto_svg.html"),
            "<html><body></body></html>",
        )
        .expect("anchor goto svg");
        fs::write(
            root.join("components/sidebar.html"),
            "<html><body>{children}</body></html>",
        )
        .expect("sidebar");
        fs::write(
            root.join("components/sidebar_each.html"),
            "<html><body>{title}</body></html>",
        )
        .expect("sidebar each");
        fs::write(
            root.join("components/embed.html"),
            "<html><body>{content}</body></html>",
        )
        .expect("embed");
        fs::write(
            root.join("components/embed_title.html"),
            "<html><body>{title}</body></html>",
        )
        .expect("embed title");
        fs::write(
            root.join("schemas/main.html"),
            "<html><body><content></content></body></html>",
        )
        .expect("schema");
    }

    pub(crate) fn run_meta_contents_init_replacement_handles_root_slug_and_parent_defaults() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test timestamp should be available")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("typsite-meta-contents-{unique}"));
        let typst = root.join("typst");
        let html = root.join("html");
        write_test_config(
            &root,
            r#"
[default_metadata.content]
title = "Fallback Title"

[default_metadata.options]
heading_numbering = "Bullet"
sidebar_type = "All"

[default_metadata.graph]
parent = "/"

[typst_lib]
paths = []

[code_fallback_style]
dark = "dark"
light = "light"
"#,
        );
        fs::create_dir_all(&typst).expect("typst dir");
        fs::create_dir_all(&html).expect("html dir");

        let config = TypsiteConfig::load(&root, &typst, &html).expect("config should load");
        let proj = ProjOptions::load(&root).expect("project options should load");
        init_proj_options(proj).expect("project options should initialize");
        init_compile_options(CompileOptions {
            watch: false,
            short_slug: true,
            pretty_url: true,
        })
        .expect("compile options should initialize");

        let slug: crate::compile::registry::Key = Arc::from("/");
        let metadata = Metadata {
            contents: MetaContents::new(slug.clone(), HashMap::new(), false),
            options: MetaOptions {
                heading_numbering_style: crate::ir::article::sidebar::HeadingNumberingStyle::Bullet,
                sidebar_type: crate::ir::article::sidebar::SidebarType::All,
            },
            node: MetaNode {
                slug: slug.clone(),
                parent: None,
                parents: HashSet::new(),
                backlinks: HashSet::new(),
                references: HashSet::new(),
                children: HashSet::new(),
            },
        };
        let article = Article::new(
            slug.clone(),
            Arc::from(std::path::Path::new("index.typ")),
            metadata,
            config.schemas.get("main").expect("schema should exist"),
            Vec::new(),
            Body::new(Vec::new(), Vec::new(), HashMap::new()),
            Sidebar::new(
                Vec::new(),
                HashSet::new(),
                HashSet::new(),
                HashMap::new(),
                HashMap::new(),
            ),
            Sidebar::new(
                Vec::new(),
                HashSet::new(),
                HashSet::new(),
                HashMap::new(),
                HashMap::new(),
            ),
            Vec::new(),
            Dependency::new(HashMap::new()),
            HashSet::new(),
            Vec::new(),
        );
        let articles = HashMap::from([(slug.clone(), article)]);
        let global_data = GlobalData::new(
            &config,
            &articles,
            HashMap::<_, OnceLock<_>>::new(),
            HashMap::from([(slug.clone(), Indexes::All)]),
            HashMap::from([(slug.clone(), Indexes::All)]),
        );
        let contents = &articles
            .get(&slug)
            .expect("article should exist")
            .get_metadata()
            .contents;

        contents.init_parent(&global_data);
        let replaced = contents.inline_with(
            "slug={slug-display}; anchor={slug@anchor}; parent={has-parent}; title={title}; page={page-title}",
            &[],
        );

        assert_eq!(
            replaced,
            "slug=/; anchor=; parent=false; title=Fallback Title; page=Fallback Title"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn meta_contents_init_replacement_handles_root_slug_and_parent_defaults() {
        let _guard = crate::test_lock();
        run_meta_contents_init_replacement_handles_root_slug_and_parent_defaults();
    }
}
