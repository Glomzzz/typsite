use crate::compile::error::{TypError, TypResult};
use crate::compile::registry::{Key, KeyRegistry, SlugPath};
use crate::config::TypsiteConfig;
use crate::config::schema::Schema;
use crate::ir::article::sidebar::Sidebar;
use crate::ir::embed::{Embed, PureEmbed};
use crate::ir::metadata::content::MetaContents;
use crate::ir::metadata::graph::MetaNode;
use crate::ir::metadata::options::MetaOptions;
use crate::ir::metadata::{Metadata, PureMetadata};
use crate::ir::pending::{AnchorData, Pending};
use crate::util::html::{OutputHead, OutputHtml};
use anyhow::{Context, Result};
use body::{Body, PureBody};
use data::GlobalData;
use dep::{Dependency, PureDependency, UpdatedIndex};
use serde::{Deserialize, Serialize, Serializer};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

pub mod body;
pub mod data;
pub mod dep;
pub mod sidebar;

struct Cache<'a> {
    // body, sidebar
    content: OnceLock<(Vec<String>, Vec<String>, Vec<String>)>,
    backlink: OnceLock<OutputHtml<'a>>,
    reference: OnceLock<OutputHtml<'a>>,
    all_used_rules: OnceLock<HashSet<&'a str>>,
    html_head: OnceLock<OutputHead<'a>>,
}
impl<'a> Cache<'a> {
    fn new() -> Cache<'a> {
        Cache {
            content: OnceLock::new(),
            html_head: OnceLock::new(),
            all_used_rules: OnceLock::new(),
            backlink: OnceLock::new(),
            reference: OnceLock::new(),
        }
    }
}

pub struct Article<'a> {
    // Article path (with extension)
    pub path: SlugPath,
    // Article slug (URL)
    pub slug: Key,
    pub schema: &'a Schema,
    pub head: Vec<String>,
    metadata: Metadata<'a>,
    body: Body<'a>,
    full_sidebar: Sidebar,
    embed_sidebar: Sidebar,
    anchors: Vec<AnchorData>,
    embeds: Vec<Embed>,
    dependency: Dependency,
    used_rules: HashSet<&'a str>,
    cache: Cache<'a>,
}

impl<'c, 'b: 'c, 'a: 'b> Article<'a> {
    pub fn from(
        pure: PureArticle,
        config: &'a TypsiteConfig,
        registry: &KeyRegistry,
    ) -> TypResult<Article<'a>> {
        let self_slug = registry.know(pure.slug, "Article", "").unwrap();
        let path = registry.path(self_slug.as_ref()).unwrap();
        let mut err = TypError::new(self_slug.clone());
        let schema = config.schemas.get(&pure.schema);
        let schema = err.ok(schema);
        let metadata = Metadata::from(self_slug.clone(), pure.metadata, config, registry);
        let metadata = err.ok_typ(metadata);
        let full_sidebar = pure.full_sidebar;
        let embed_sidebar = pure.embed_sidebar;
        let head = pure.head;
        let body = Body::from(self_slug.clone(), pure.body, config);
        let body = err.ok_typ(body);
        let embeds = pure
            .embeds
            .into_iter()
            .map(|embed| err.ok(Embed::from(self_slug.as_str(), embed, registry)))
            .collect::<Vec<Option<_>>>();
        let dependency = Dependency::from(self_slug.clone(), pure.dependency, config, registry);
        let dependency = err.ok_typ(dependency);
        let used_rules = pure
            .used_rules
            .into_iter()
            .map(|rule| {
                err.ok(config
                    .rules
                    .rule_name(rule.as_str())
                    .with_context(|| format!("No rewrite rule named {rule}")))
            })
            .collect::<Vec<Option<_>>>();
        let anchors = pure.anchors;
        if err.has_error() {
            return Err(err);
        }
        let metadata = metadata.unwrap();
        let schema = schema.unwrap();
        let body = body.unwrap();
        let embeds = embeds.into_iter().flatten().collect();
        let dependency = dependency.unwrap();
        let used_rules = used_rules.into_iter().flatten().collect();
        let article = Article {
            slug: self_slug,
            path,
            metadata,
            schema,
            head,
            full_sidebar,
            embed_sidebar,
            body,
            embeds,
            dependency,
            used_rules,
            anchors,
            cache: Cache::new(),
        };
        Ok(article)
    }

    pub fn new(
        slug: Key,
        path: SlugPath,
        metadata: Metadata<'a>,
        schema: &'a Schema,
        head: Vec<String>,
        body: Body<'a>,
        full_sidebar: Sidebar,
        embed_sidebar: Sidebar,
        embeds: Vec<Embed>,
        dependency: Dependency,
        used_rules: HashSet<&'a str>,
        anchors: Vec<AnchorData>,
    ) -> Self {
        Article {
            slug,
            path,
            metadata,
            schema,
            head,
            full_sidebar,
            embed_sidebar,
            body,
            embeds,
            dependency,
            used_rules,
            anchors,
            cache: Cache::new(),
        }
    }

    pub fn get_content_or_init(
        &'b self,
        global_data: &'c GlobalData<'a, 'b, 'c>,
    ) -> &'b (Vec<String>, Vec<String>, Vec<String>) {
        self.cache
            .content
            .get_or_init(|| global_data.init_cache(self))
    }

    pub fn get_pending_or_init(
        &'b self,
        global_data: &'c GlobalData<'a, 'b, 'c>,
    ) -> &'c Pending<'c> {
        global_data.get_pending_or_init(self)
    }

    pub fn get_body(&self) -> &Body<'a> {
        &self.body
    }

    pub fn get_full_sidebar(&self) -> &Sidebar {
        &self.full_sidebar
    }

    pub fn get_embed_sidebar(&self) -> &Sidebar {
        &self.embed_sidebar
    }

    pub fn get_metadata(&'b self) -> &'b Metadata<'a> {
        &self.metadata
    }

    pub fn get_meta_options(&self) -> &MetaOptions {
        &self.metadata.options
    }

    pub fn get_meta_contents(&self) -> &MetaContents<'a> {
        &self.metadata.contents
    }

    pub fn get_meta_node(&self) -> &MetaNode {
        &self.metadata.node
    }

    pub fn get_mut_meta_node(&mut self) -> &mut MetaNode {
        &mut self.metadata.node
    }


    pub fn all_used_rules(&self, global_data: &'c GlobalData<'a, 'b, 'c>) -> &HashSet<&'a str> {
        self.cache.all_used_rules.get_or_init(|| {
            let mut all_used_rules = self.used_rules.clone();
            self.metadata.node.children.iter().for_each(|child| {
                if let Some(child) = global_data.article(child.as_str()) {
                    all_used_rules.extend(child.all_used_rules(global_data));
                } else {
                    eprintln!(
                        "[WARN] (all_used_rules) Embed article {} not found in {} ",
                        child, self.slug
                    );
                }
            });
            all_used_rules
        })
    }

    pub fn get_depending_articles(&self) -> HashSet<Key> {
        self.dependency.articles()
    }

    pub fn get_dependency(
        &self,
        registry: &KeyRegistry,
    ) -> HashMap<Arc<Path>, HashSet<UpdatedIndex>> {
        self.dependency.unwrap(registry)
    }

    pub fn get_depending_components(&self, config: &'a TypsiteConfig) -> HashSet<Arc<Path>> {
        let mut components = self.schema.component_paths(config);
        if !self.metadata.node.children.is_empty() {
            components.insert(config.embed.embed_path.clone());
            components.insert(config.embed.embed_title_path.clone());
        }
        components
    }

    pub fn get_backlink(&self) -> Option<&OutputHtml<'a>> {
        self.cache.backlink.get()
    }

    pub fn get_reference(&self) -> Option<&OutputHtml<'a>> {
        self.cache.reference.get()
    }

    pub fn get_anchors(&'b self) -> &'b Vec<AnchorData> {
        &self.anchors
    }
}

unsafe impl Send for Article<'_> {}

#[derive(Debug, Serialize, Deserialize)]
pub struct PureArticle {
    pub(crate) path: PathBuf,
    slug: String,
    #[serde(rename = "$schema")]
    schema: String,
    metadata: PureMetadata,
    head: Vec<String>,

    body: PureBody,
    full_sidebar: Sidebar,
    embed_sidebar: Sidebar,

    embeds: Vec<PureEmbed>,
    dependency: PureDependency,
    #[serde(serialize_with = "ordered_set")]
    used_rules: HashSet<String>,
    anchors: Vec<AnchorData>,
}

impl PureArticle {
    pub fn from(
        article: Article<'_>,
        content_cache: (Vec<String>, Vec<String>, Vec<String>),
    ) -> PureArticle {
        let slug = article.slug.to_string();
        let path = article.path.to_path_buf();
        let metadata = article.metadata;
        let (body_content, full_sidebar, embed_sidebar) = content_cache;
        let body = Body::new(
            body_content,
            article.body.rewriters,
            article.body.numberings,
        );
        let body = PureBody::from(body);
        let full_sidebar = article.full_sidebar.with_contents(full_sidebar);
        let embed_sidebar = article.embed_sidebar.with_contents(embed_sidebar);
        let schema = article.schema.id.clone();
        let metadata = PureMetadata::from(metadata);
        let head = article.head;
        let embeds = article.embeds.into_iter().map(PureEmbed::from).collect();
        let dependency = PureDependency::from(article.dependency);
        let used_rules = article
            .cache
            .all_used_rules
            .into_inner()
            .unwrap_or(article.used_rules)
            .into_iter()
            .map(str::to_string)
            .collect();
        let anchors = article.anchors;
        PureArticle {
            slug,
            path,
            schema,
            metadata,
            head,
            full_sidebar,
            embed_sidebar,
            body,
            embeds,
            dependency,
            used_rules,
            anchors,
        }
    }
}

fn ordered_set<S>(value: &HashSet<String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut vec: Vec<_> = value.iter().collect();
    vec.sort();
    serializer.collect_seq(vec)
}
