use crate::compile::error::{TypError, TypResult};
use crate::config::footer::{BACKLINKS_KEY, REFERENCES_KEY};
use crate::config::schema::{Schema, BACKLINK_KEY, REFERENCE_KEY};
use crate::config::TypsiteConfig;
use crate::ir::article::data::GlobalData;
use crate::ir::article::Article;
use crate::util::error::TypsiteError;
use crate::util::html::{write_token, OutputHead};
use crate::util::html::{Attributes, OutputHtml};
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
        let metadata = self
            .global_data
            .metadata(self.article.slug.as_ref())
            .unwrap();

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
            let references = node
                .references
                .iter()
                .filter_map(|slug| self.global_data.article(slug))
                .filter_map(|article| article.get_reference())
                .collect::<Vec<_>>();

            let backlinks = node
                .backlinks
                .iter()
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
        // Body
        let tokenizer = Tokenizer::new(&body);
        let mut err = TypError::new_schema(self.article.slug.clone(), self.schema.id.as_str());
        for result in tokenizer {
            match result {
                Ok(Token::StartTag(tag)) if tag.name == b"metadata" => {
                    let attrs = Attributes::new(tag.attributes);
                    let meta_key = attrs
                        .expect("get")
                        .context("Expect Metadata tag with attr `get`");
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
                Ok(Token::StartTag(tag)) if tag.name == b"content" => {
                    err.ok(write_into!(self.body, "{}\n", self.content))
                }
                Ok(Token::StartTag(tag)) if tag.name == b"footer" => {
                    err.ok(write_into!(self.body, "{}\n", footer.body))
                }
                Ok(Token::EndTag(tag)) => match tag.name.as_slice() {
                    b"metadata" | b"sidebar" | b"content" | b"footer" => None,
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
