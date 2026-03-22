use anyhow::anyhow;

use crate::compile::compile_options;
use crate::compile::error::{TypError, TypResult};
use crate::compile::registry::Key;
use crate::compile::watch::WATCH_AUTO_RELOAD_SCRIPT;
use crate::config::schema::{BACKLINK_KEY, REFERENCE_KEY};
use crate::config::TypsiteConfig;
use crate::ir::article::sidebar::HeadingNumberingStyle;
use crate::ir::metadata::Metadata;
use crate::ir::pending::Pending;
use crate::pass::{pass_embed, pass_rewriter_body, pass_schema};
use crate::util::html::{OutputHead, OutputHtml};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use super::dep::Indexes;
use super::Article;

pub struct GlobalData<'a, 'b, 'c> {
    pub config: &'a TypsiteConfig<'a>,
    pub articles: &'b HashMap<Key, Article<'a>>,
    pendings: HashMap<Key, OnceLock<Pending<'c>>>,
    global_body_rewrite_indexes: HashMap<Key, Indexes>,
    global_body_embed_indexes: HashMap<Key, Indexes>,
    runtime_errors: Mutex<HashMap<Key, TypError>>,
    invalid_pending: OnceLock<Pending<'c>>,
}
impl<'c, 'b: 'c, 'a: 'b> GlobalData<'a, 'b, 'c> {
    pub fn new(
        config: &'a TypsiteConfig<'a>,
        articles: &'b HashMap<Key, Article<'a>>,
        pendings: HashMap<Key, OnceLock<Pending<'c>>>,
        global_body_rewrite_indexes: HashMap<Key, Indexes>,
        global_body_embed_indexes: HashMap<Key, Indexes>,
    ) -> Self {
        Self {
            config,
            articles,
            pendings,
            global_body_rewrite_indexes,
            global_body_embed_indexes,
            runtime_errors: Mutex::new(HashMap::new()),
            invalid_pending: OnceLock::new(),
        }
    }

    fn record_runtime_error(&self, slug: Key, err: anyhow::Error) {
        let mut errors = self
            .runtime_errors
            .lock()
            .expect("global runtime error cache should not be poisoned");
        errors
            .entry(slug.clone())
            .or_insert_with(|| TypError::new(slug))
            .add(err);
    }

    fn invalid_pending(&'c self) -> &'c Pending<'c> {
        self.invalid_pending.get_or_init(|| {
            let raw = Box::leak(Box::new((Vec::new(), Vec::new(), Vec::new())));
            let anchors = Box::leak(Box::new(Vec::new()));
            Pending::new(
                raw,
                HeadingNumberingStyle::Bullet,
                Vec::new(),
                crate::ir::pending::SidebarData::new(
                    crate::ir::pending::SidebarIndexesData::new(HashSet::new()),
                    Vec::new(),
                    Vec::new(),
                ),
                crate::ir::pending::SidebarData::new(
                    crate::ir::pending::SidebarIndexesData::new(HashSet::new()),
                    Vec::new(),
                    Vec::new(),
                ),
                Vec::new(),
                anchors,
            )
        })
    }

    #[cfg(test)]
    pub(crate) fn take_runtime_error(&self, id: &str) -> Option<TypError> {
        self.runtime_errors
            .lock()
            .expect("global runtime error cache should not be poisoned")
            .remove(id)
    }

    pub fn article(&'c self, id: &str) -> Option<&'b Article<'a>> {
        self.articles.get(id)
    }

    pub fn metadata(&'c self, id: &str) -> Option<&'b Metadata<'a>> {
        let article = self.article(id)?;
        Some(article.get_metadata())
    }

    pub(super) fn init_cache(
        &'c self,
        article: &'b Article<'a>,
    ) -> (Vec<String>, Vec<String>, Vec<String>) {
        let metadata = &article.metadata;
        let mut body = article.body.clone();
        let mut full_sidebar = article.full_sidebar.cache(metadata);
        let embed_sidebar = article.embed_sidebar.cache(metadata);
        let Some(rewriter_indexes) = self.global_body_rewrite_indexes.get(article.slug.as_ref())
        else {
            self.record_runtime_error(
                article.slug.clone(),
                anyhow!(
                    "Missing global body rewrite indexes for loaded article {}",
                    article.slug
                ),
            );
            return (body.content, full_sidebar, embed_sidebar);
        };
        pass_rewriter_body(
            article.slug.clone(),
            &mut body.content,
            &mut full_sidebar,
            &body.rewriters,
            rewriter_indexes,
            self,
        );
        (body.content, full_sidebar, embed_sidebar)
    }

    pub(super) fn get_pending_or_init(&'c self, article: &'b Article<'a>) -> &'c Pending<'c> {
        let Some(pending) = self.pendings.get(article.slug.as_ref()) else {
            self.record_runtime_error(
                article.slug.clone(),
                anyhow!("Missing pending cache for loaded article {}", article.slug),
            );
            return self.invalid_pending();
        };

        pending.get_or_init(|| {
            let Some(embed_indexes) = self.global_body_embed_indexes.get(article.slug.as_ref())
            else {
                self.record_runtime_error(
                    article.slug.clone(),
                    anyhow!(
                        "Missing global body embed indexes for loaded article {}",
                        article.slug
                    ),
                );
                return Pending::new(
                    Box::leak(Box::new((Vec::new(), Vec::new(), Vec::new()))),
                    article.get_meta_options().heading_numbering_style,
                    Vec::new(),
                    crate::ir::pending::SidebarData::new(
                        crate::ir::pending::SidebarIndexesData::new(HashSet::new()),
                        Vec::new(),
                        Vec::new(),
                    ),
                    crate::ir::pending::SidebarData::new(
                        crate::ir::pending::SidebarIndexesData::new(HashSet::new()),
                        Vec::new(),
                        Vec::new(),
                    ),
                    Vec::new(),
                    Box::leak(Box::new(Vec::new())),
                );
            };
            let content = article.get_content_or_init(self);
            pass_embed(
                article.slug.clone(),
                content,
                &article.embeds,
                embed_indexes,
                self,
            )
        })
    }
    pub fn schema_html(
        &'c self,
        schema_id: &str,
        article: &'b Article<'a>,
        content: &str,
        sidebar: &str,
    ) -> TypResult<OutputHtml<'a>> {
        let schema = self.config.schemas.get(schema_id);
        match schema {
            Err(_) => {
                let mut err = TypError::new_schema(article.slug.clone(), schema_id);
                err.add(anyhow!("Shchema {schema_id} not found"));
                Err(err)
            }
            Ok(schema) => pass_schema(self.config, schema, article, content, sidebar, self),
        }
    }

    pub fn init_backlink(
        &'c self,
        article: &'b Article<'a>,
        content: &str,
        sidebar: &str,
    ) -> TypResult<()> {
        let backlink = self.schema_html(BACKLINK_KEY, article, content, sidebar)?;
        article.cache.backlink.set(backlink).map_err(|_| {
            let err = anyhow::anyhow!("Failed to set backlink");
            TypError::new_with(article.slug.clone(), vec![err])
        })
    }
    pub fn init_reference(
        &'c self,
        article: &'b Article<'a>,
        content: &str,
        sidebar: &str,
    ) -> TypResult<()> {
        let reference = self.schema_html(REFERENCE_KEY, article, content, sidebar)?;
        article.cache.reference.set(reference).map_err(|_| {
            let err = anyhow::anyhow!("Failed to set reference");
            TypError::new_with(article.slug.clone(), vec![err])
        })
    }

    pub fn init_component_head(&'c self, article: &'b Article<'a>, head: &mut OutputHead<'a>) {
        let schema = article.schema;
        let metadata = article.get_metadata();
        head.push(self.config.section.head.as_str());
        head.push(self.config.heading_numbering.head.as_str());

        if schema.sidebar {
            head.push(self.config.sidebar.each.head.as_str());
            head.push(self.config.sidebar.block.head.as_str());
        }

        if !metadata.node.children.is_empty() {
            head.push(self.config.embed.embed.head.as_str());
            head.push(self.config.embed.embed_title.head.as_str());
        }

        if !article.get_anchors().is_empty() {
            head.push(self.config.anchor.define.head.as_str());
            head.push(self.config.anchor.goto.head.as_str());
        }
    }
    pub fn init_rewrite_head(&'c self, article: &'b Article<'a>, head: &mut OutputHead<'a>) {
        let metadata = article.get_metadata();
        let mut rules = article.all_used_rules(self).clone();

        rules.extend(
            metadata
                .node
                .refs_and_backlinks()
                .into_iter()
                .filter_map(|slug| self.article(slug))
                .map(|article| article.all_used_rules(self))
                .flatten(),
        );
        {
            let mut heads = HashSet::new();
            for rule_id in rules.iter() {
                if let Some(rule) = self.config.rules.get(rule_id) {
                    heads.insert(&rule.head);
                }
            }
            for rule_head in heads {
                head.push(rule_head.as_str());
            }
        }
    }

    pub fn init_article_head(&'c self, article: &'b Article<'a>, head: &mut OutputHead<'a>) {
        let metadata = article.get_metadata();
        article.head.iter().for_each(|it| head.end(it.to_string()));
        metadata
            .node
            .children
            .iter()
            .filter_map(|it| self.article(it))
            .for_each(|it| self.init_article_head(it, head));
    }

    pub fn init_html_head(&'c self, article: &'b Article<'a>) -> &'b OutputHead<'a> {
        article.cache.html_head.get_or_init(|| {
            let metadata = article.get_metadata();
            let schema = article.schema;

            let mut head = OutputHead::empty();
            // Head
            head.start(metadata.inline(schema.head.as_str()));

            self.init_component_head(article, &mut head);
            metadata
                .node
                .refs_and_backlinks()
                .into_iter()
                .filter_map(|slug| self.article(slug))
                .for_each(|article| self.init_component_head(article, &mut head));

            if compile_options()
                .map(|options| options.watch)
                .unwrap_or(false)
            {
                head.push(WATCH_AUTO_RELOAD_SCRIPT);
            }

            self.init_rewrite_head(article, &mut head);
            self.init_article_head(article, &mut head);

            head
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::compile::registry::{Key, SlugPath};
    use crate::config::TypsiteConfig;
    use crate::ir::article::body::Body;
    use crate::ir::article::dep::Dependency;
    use crate::ir::article::sidebar::{HeadingNumberingStyle, Sidebar, SidebarType};
    use crate::ir::article::Article;
    use crate::ir::metadata::content::{MetaContent, MetaContents, TITLE_KEY};
    use crate::ir::metadata::graph::MetaNode;
    use crate::ir::metadata::options::MetaOptions;
    use crate::ir::metadata::Metadata;
    use crate::ir::pending::AnchorData;
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test timestamp should be available")
            .as_nanos();
        std::env::temp_dir().join(format!("typsite-{label}-{unique}"))
    }

    fn write_test_config(root: &Path) {
        fs::create_dir_all(root.join("components/footer")).expect("footer dir");
        fs::create_dir_all(root.join("rewrite")).expect("rewrite dir");
        fs::create_dir_all(root.join("schemas")).expect("schema dir");
        fs::create_dir_all(root.join("syntaxes")).expect("syntaxes dir");
        fs::create_dir_all(root.join("themes")).expect("themes dir");

        fs::write(
            root.join("options.toml"),
            include_str!("../../../resources/.typsite/options.toml"),
        )
        .expect("options file");
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
        .expect("main schema");
        fs::write(
            root.join("schemas/reference.html"),
            "<html><body><content></content></body></html>",
        )
        .expect("reference schema");
    }

    fn article_with_title<'a>(config: &'a TypsiteConfig<'a>, slug: &str) -> Article<'a> {
        let slug = Key::from(slug);
        let mut contents = HashMap::new();
        contents.insert(
            TITLE_KEY.to_string(),
            MetaContent::new(vec!["Regression Article".to_string()], Vec::new()),
        );
        let metadata = Metadata {
            contents: MetaContents::new(slug.clone(), contents, false),
            options: MetaOptions {
                heading_numbering_style: HeadingNumberingStyle::Bullet,
                sidebar_type: SidebarType::All,
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

        Article::new(
            slug,
            SlugPath::from(PathBuf::from("article.typ")),
            metadata,
            config.schemas.get("main").expect("main schema should load"),
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
            Vec::<AnchorData>::new(),
        )
    }

    pub(crate) fn run_global_data_reports_missing_rewrite_indexes() {
        let root = unique_root("global-data-missing-rewrite-indexes");
        let typst = root.join("typst");
        let html = root.join("html");
        write_test_config(&root);
        fs::create_dir_all(&typst).expect("typst dir");
        fs::create_dir_all(&html).expect("html dir");

        let config = TypsiteConfig::load(&root, &typst, &html).expect("config should load");
        let slug = Key::from("/article");
        let articles = HashMap::from([(slug.clone(), article_with_title(&config, "/article"))]);
        let global_data = GlobalData::new(
            &config,
            &articles,
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
        );
        let article = global_data
            .article("/article")
            .expect("article should exist in test global data");

        let content = article.get_content_or_init(&global_data);
        let error = global_data
            .take_runtime_error("/article")
            .expect("missing rewrite indexes should be recorded");

        assert!(content.0.is_empty());
        assert!(format!("{error}").contains("Missing global body rewrite indexes"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn global_data_reports_missing_rewrite_indexes() {
        let _guard = crate::test_lock();
        run_global_data_reports_missing_rewrite_indexes();
    }
}
