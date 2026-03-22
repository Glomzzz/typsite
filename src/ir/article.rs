use crate::compile::error::{TypError, TypResult};
use crate::compile::registry::{Key, KeyRegistry, SlugPath};
use crate::config::schema::Schema;
use crate::config::TypsiteConfig;
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
        let self_slug = registry
            .slug(pure.slug.as_str())
            .unwrap_or_else(|| Arc::from(pure.slug.as_str()));
        let mut err = TypError::new(self_slug.clone());
        let path = err.ok(registry
            .path(self_slug.as_ref())
            .with_context(|| format!("Article path not found for {}", self_slug)));
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
            .map(|embed| err.ok(Embed::from(self_slug.as_ref(), embed, registry)))
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
        let path = path.ok_or_else(|| {
            TypError::new_with(
                self_slug.clone(),
                vec![anyhow::anyhow!("Article path missing after validation")],
            )
        })?;
        let metadata = metadata.ok_or_else(|| {
            TypError::new_with(
                self_slug.clone(),
                vec![anyhow::anyhow!("Metadata missing after validation")],
            )
        })?;
        let schema = schema.ok_or_else(|| {
            TypError::new_with(
                self_slug.clone(),
                vec![anyhow::anyhow!("Schema missing after validation")],
            )
        })?;
        let body = body.ok_or_else(|| {
            TypError::new_with(
                self_slug.clone(),
                vec![anyhow::anyhow!("Body missing after validation")],
            )
        })?;
        let embeds = embeds.into_iter().flatten().collect();
        let dependency = dependency.ok_or_else(|| {
            TypError::new_with(
                self_slug.clone(),
                vec![anyhow::anyhow!("Dependency missing after validation")],
            )
        })?;
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
                if let Some(child) = global_data.article(child.as_ref()) {
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::compile::{
        init_compile_options, init_proj_options,
        options::{CompileOptions, ProjOptions},
    };
    use crate::config::TypsiteConfig;
    use crate::ir::article::body::Body;
    use crate::ir::article::dep::Dependency;
    use crate::ir::metadata::options::MetaOptions;
    use crate::ir::metadata::{Metadata, PureMetadata};
    use std::fs;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_test_config(root: &std::path::Path) {
        fs::create_dir_all(root.join("components/footer")).expect("footer dir");
        fs::create_dir_all(root.join("rewrite")).expect("rewrite dir");
        fs::create_dir_all(root.join("schemas")).expect("schema dir");
        fs::create_dir_all(root.join("syntaxes")).expect("syntaxes dir");
        fs::create_dir_all(root.join("themes")).expect("themes dir");

        fs::write(
            root.join("options.toml"),
            include_str!("../../resources/.typsite/options.toml"),
        )
        .expect("options file");
        fs::write(
            root.join("components/section.html"),
            include_str!("../../resources/.typsite/components/section.html"),
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
            root.join("schemas/reference.html"),
            "<html><body><content></content></body></html>",
        )
        .expect("reference schema");
    }

    pub(crate) fn run_article_from_collects_registry_and_schema_errors() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test timestamp should be available")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("typsite-article-from-{unique}"));
        let typst = root.join("typst");
        let html = root.join("html");
        write_test_config(&root);
        fs::create_dir_all(&typst).expect("typst dir");
        fs::create_dir_all(&html).expect("html dir");

        let config = TypsiteConfig::load(&root, &typst, &html).expect("config should load");
        let proj = ProjOptions::load(&root).expect("project options should load");
        init_proj_options(proj).expect("project options should initialize");
        init_compile_options(CompileOptions {
            watch: false,
            short_slug: false,
            pretty_url: true,
        })
        .expect("compile options should initialize");

        let pure = PureArticle {
            path: std::path::PathBuf::from("missing.typ"),
            slug: "/missing".to_string(),
            schema: "missing-schema".to_string(),
            metadata: PureMetadata::from(Metadata {
                contents: crate::ir::metadata::content::MetaContents::new(
                    Arc::from("/missing"),
                    HashMap::new(),
                    false,
                ),
                options: MetaOptions {
                    heading_numbering_style:
                        crate::ir::article::sidebar::HeadingNumberingStyle::Bullet,
                    sidebar_type: crate::ir::article::sidebar::SidebarType::All,
                },
                node: crate::ir::metadata::graph::MetaNode {
                    slug: Arc::from("/missing"),
                    parent: Some(Arc::from("/missing-parent")),
                    parents: HashSet::new(),
                    backlinks: HashSet::new(),
                    references: HashSet::new(),
                    children: HashSet::new(),
                },
            }),
            head: Vec::new(),
            body: body::PureBody::from(Body::new(Vec::new(), Vec::new(), HashMap::new())),
            full_sidebar: Sidebar::new(
                Vec::new(),
                HashSet::new(),
                HashSet::new(),
                HashMap::new(),
                HashMap::new(),
            ),
            embed_sidebar: Sidebar::new(
                Vec::new(),
                HashSet::new(),
                HashSet::new(),
                HashMap::new(),
                HashMap::new(),
            ),
            embeds: Vec::new(),
            dependency: crate::ir::article::dep::PureDependency::from(Dependency::new(
                HashMap::new(),
            )),
            used_rules: HashSet::new(),
            anchors: Vec::new(),
        };
        let registry = KeyRegistry::new();

        let err = match Article::from(pure, &config, &registry) {
            Ok(_) => panic!("missing registry path and schema should be aggregated"),
            Err(err) => err,
        };
        let err = err.to_string();

        assert!(err.contains("Article path not found for /missing"));
        assert!(err.contains("No schema named missing-schema"));
        assert!(err.contains("Parent not found: /missing-parent in /missing"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn article_from_collects_registry_and_schema_errors() {
        let _guard = crate::test_lock();
        run_article_from_collects_registry_and_schema_errors();
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
