use crate::compile::error::{TypError, TypResult};
use crate::config::TypsiteConfig;
use crate::config::footer::{BACKLINKS_KEY, REFERENCES_KEY};
use crate::config::schema::{BACKLINK_KEY, REFERENCE_KEY, Schema};
use crate::ir::article::Article;
use crate::ir::article::data::GlobalData;
use crate::util::error::TypsiteError;
use crate::util::html::{Attributes, OutputHtml};
use crate::util::html::{OutputHead, write_token};
use crate::util::str::ac_replace;
use crate::write_into;
use anyhow::Context;
use html5gum::{Token, Tokenizer};
use std::borrow::Cow;
use std::fmt::Write;

pub struct SchemaPass<'a, 'b, 'c, 'd> {
    config: &'a TypsiteConfig<'a>,
    schema: &'a Schema,
    article: &'c Article<'a>,
    body: String,
    sidebar: &'d str,
    content: &'d str,
    global_data: &'c GlobalData<'a, 'b, 'c>,
}

impl<'d, 'c: 'd, 'b: 'c, 'a: 'b> SchemaPass<'a, 'b, 'c, 'd> {
    pub fn new(
        config: &'a TypsiteConfig,
        schema: &'a Schema,
        article: &'c Article<'a>,
        content: &'d str,
        sidebar: &'d str,
        global_data: &'c GlobalData<'a, 'b, 'c>,
    ) -> Self {
        Self {
            config,
            schema,
            global_data,
            article,
            sidebar,
            content,
            body: String::new(),
        }
    }

    pub fn run(mut self) -> TypResult<OutputHtml<'a>> {
        let mut err = TypError::new_schema(self.article.slug.clone(), self.schema.id.as_str());
        let metadata = err.ok(self
            .global_data
            .metadata(self.article.slug.as_ref())
            .with_context(|| format!("Metadata of {} not found", self.article.slug)));
        if err.has_error() {
            return Err(err);
        }
        let Some(metadata) = metadata else {
            let err = anyhow::anyhow!("Metadata unexpectedly missing for {}", self.article.slug);
            return Err(TypError::new_with(self.article.slug.clone(), vec![err]));
        };

        let footer_schema = matches!(self.schema.id.as_str(), REFERENCE_KEY | BACKLINK_KEY);

        let head = if self.schema.content {
            self.global_data.init_html_head(self.article).clone()
        } else {
            OutputHead::empty()
        };

        let has_footer = !footer_schema && self.schema.footer;

        let footer = if has_footer {
            let mut footer = OutputHtml::empty();
            footer.head.push(self.config.footer.footer.head.as_str());
            let footer_body = self.config.footer.footer.body.as_str();
            let node = self.article.get_meta_node();
            let descendants = self.global_data.descendant_slugs(self.article);
            let mut references: Vec<_> = node.references.iter().collect();
            references.sort();
            let references = references
                .into_iter()
                .filter(|slug| !descendants.contains(*slug))
                .filter_map(|slug| self.global_data.article(slug))
                .filter_map(|article| article.get_reference())
                .collect::<Vec<_>>();
            let mut backlinks: Vec<_> = node.backlinks.iter().collect();
            backlinks.sort();
            let backlinks = backlinks
                .into_iter()
                .filter(|slug| !descendants.contains(*slug))
                .filter_map(|slug| self.global_data.article(slug))
                .filter_map(|article| article.get_backlink())
                .collect::<Vec<_>>();

            let has_references = !references.is_empty();
            let has_backlinks = !backlinks.is_empty();

            fn footer_component_html<'b, 'a: 'b>(
                footer_body: &str,
                key: &str,
                component: Vec<&'b OutputHtml<'a>>,
            ) -> OutputHtml<'a> {
                if footer_body.contains(key) && !component.is_empty() {
                    component
                        .into_iter()
                        .fold(OutputHtml::empty(), |mut acc, x| {
                            acc.extend_body(x);
                            acc
                        })
                } else {
                    OutputHtml::empty()
                }
            }

            let backlinks = footer_component_html(footer_body, BACKLINKS_KEY, backlinks);
            let references = footer_component_html(footer_body, REFERENCES_KEY, references);

            let backlinks = if has_backlinks {
                ac_replace(
                    &self.config.footer.backlinks.body,
                    &[(BACKLINKS_KEY, &backlinks.body)],
                )
            } else {
                String::default()
            };
            let references = if has_references {
                ac_replace(
                    &self.config.footer.references.body,
                    &[(REFERENCES_KEY, &references.body)],
                )
            } else {
                String::default()
            };
            footer.body = ac_replace(
                footer_body,
                &[(REFERENCES_KEY, &references), (BACKLINKS_KEY, &backlinks)],
            );
            footer
        } else {
            OutputHtml::empty()
        };

        let body = metadata.inline(&self.schema.body);
        // Furniture is exported as reserved native metadata, not body content:
        // embeds reuse their article bodies without duplicating page furniture.
        let furniture = |key: &str, tag: &str| {
            if !self.schema.content
                || self.config.options.project.layout.module.trim().is_empty()
                || !metadata.contents.contents.contains_key(key)
            {
                return String::new();
            }
            metadata
                .contents
                .get(key)
                .map(|content| {
                    if content.trim().is_empty() {
                        String::new()
                    } else {
                        format!(
                            "<{tag} class=\"page-{tag}\">{}</{tag}>",
                            metadata.inline(&content)
                        )
                    }
                })
                .unwrap_or_default()
        };
        let page_header = furniture("__typsite-page-header", "header");
        let page_footer = furniture("__typsite-page-footer", "footer");
        let tokens: Vec<_> = Tokenizer::new(&body).collect();
        let has_slot = |name: &[u8]| {
            tokens.iter().any(
                |token| matches!(token, Ok(Token::StartTag(tag)) if tag.name.as_slice() == name),
            )
        };
        let has_header_slot = has_slot(b"page-header");
        let has_footer_slot = has_slot(b"page-footer");
        let mut header_written = false;
        let mut footer_written = false;
        // Body
        let mut err = TypError::new_schema(self.article.slug.clone(), self.schema.id.as_str());
        for result in tokens {
            match result {
                Ok(Token::StartTag(tag)) if tag.name == b"metadata" => {
                    let attrs = Attributes::new(tag.attributes);
                    let meta_key = attrs
                        .expect("get")
                        .context("Expected <metadata> tag with required attr `get`");
                    let meta_key = err.ok(meta_key);
                    let from = attrs.get("from").unwrap_or(Cow::Borrowed("$self"));
                    let metadata = match from.as_ref() {
                        "$self" => Some(metadata),
                        from => {
                            let from = self
                                .global_data
                                .articles
                                .get(from)
                                .with_context(|| {
                                    format!("Article {from} not found in metadata's attr `from`")
                                })
                                .map(|it| it.slug.as_ref())
                                .and_then(|from| {
                                    self.global_data
                                        .metadata(from)
                                        .with_context(|| format!("Metadata of {from} not found"))
                                });
                            err.ok(from)
                        }
                    };
                    metadata.zip(meta_key).and_then(|(metadata, meta_key)| {
                        let content = metadata
                            .contents
                            .get(&meta_key)
                            .with_context(|| format!("Metadata key {meta_key} not found"));
                        err.ok(content)
                            .and_then(|content| err.ok(write_into!(self.body, "{}", content)))
                    })
                }
                Ok(Token::StartTag(tag)) if tag.name == b"sidebar" => {
                    let body = metadata.inline(self.config.sidebar.block.body.as_str());
                    let tail = metadata.inline(self.config.sidebar.block.tail.as_str());
                    err.ok(write_into!(self.body, "{body}\n{}\n{tail}", self.sidebar))
                }
                Ok(Token::StartTag(tag)) if tag.name == b"page-header" => {
                    if !header_written {
                        self.body.push_str(&page_header);
                        header_written = true;
                    }
                    None
                }
                Ok(Token::StartTag(tag)) if tag.name == b"page-footer" => {
                    if !footer_written {
                        self.body.push_str(&page_footer);
                        footer_written = true;
                    }
                    None
                }
                Ok(Token::StartTag(tag)) if tag.name == b"content" => {
                    if !has_header_slot && !header_written {
                        self.body.push_str(&page_header);
                        header_written = true;
                    }
                    err.ok(write_into!(self.body, "{}\n", self.content));
                    if !has_footer_slot && !footer_written {
                        self.body.push_str(&page_footer);
                        footer_written = true;
                    }
                    None
                }
                Ok(Token::StartTag(tag)) if tag.name == b"footer" => {
                    err.ok(write_into!(self.body, "{}\n", footer.body))
                }
                Ok(Token::EndTag(tag)) => match tag.name.as_slice() {
                    b"metadata" | b"sidebar" | b"content" | b"footer" | b"page-header"
                    | b"page-footer" => None,
                    _ => err.ok(write_token(&mut self.body, &Token::EndTag(tag))),
                },
                Ok(token) => err.ok(write_token(&mut self.body, &token)),
                Err(e) => {
                    err.add(TypsiteError::HtmlParse(e).into());
                    break;
                }
            };
        }
        if err.has_error() {
            return Err(err);
        }
        let html = OutputHtml::<'a>::new(head, self.body);
        Ok(html)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::config::TypsiteConfig;
    use crate::ir::article::body::Body;
    use crate::ir::article::data::GlobalData;
    use crate::ir::article::dep::{Dependency, Indexes};
    use crate::ir::article::{Article, sidebar::Sidebar};
    use crate::ir::metadata::Metadata;
    use crate::ir::metadata::content::MetaContents;
    use crate::ir::metadata::graph::MetaNode;
    use crate::ir::metadata::options::MetaOptions;
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::sync::OnceLock;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_test_config(root: &std::path::Path) {
        crate::test_support::write_config(root);
        fs::write(
            root.join("schemas/main.html"),
            "<head></head><body><metadata></metadata></body>",
        )
        .unwrap();
    }

    pub(crate) fn run_schema_pass_reports_missing_metadata_tag_attr() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test timestamp should be available")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("typsite-schema-pass-{unique}"));
        let typst = root.join("typst");
        let html = root.join("html");
        write_test_config(&root);
        fs::create_dir_all(&typst).expect("typst dir");
        fs::create_dir_all(&html).expect("html dir");

        let config = TypsiteConfig::load(&root, &typst, &html).expect("config should load");

        let slug: crate::compile::registry::Key = std::sync::Arc::from("/article");
        let metadata = Metadata {
            contents: MetaContents::new(slug.clone(), HashMap::new(), false, &config.options),
            options: MetaOptions {
                rss: false,
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
            std::sync::Arc::from(std::path::Path::new("article.typ")),
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

        let err = SchemaPass::new(
            &config,
            config.schemas.get("main").expect("schema should exist"),
            articles.get(&slug).expect("article should exist"),
            "",
            "",
            &global_data,
        )
        .run()
        .expect_err("missing metadata get attr should not panic");
        let err = err.to_string();

        assert!(err.contains("required attr `get`"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn schema_pass_reports_missing_metadata_tag_attr() {
        run_schema_pass_reports_missing_metadata_tag_attr();
    }
}
