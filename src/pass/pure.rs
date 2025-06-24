use crate::compile::registry::{Key, KeyRegistry, SlugPath};
use crate::config::TypsiteConfig;
use crate::config::rewrite::TagRewriteRule;
use crate::config::schema::Schema;
use crate::ir::article::Article;
use crate::ir::article::body::Body;
use crate::ir::article::dep::{Dependency, Source, UpdatedIndex};
use crate::ir::article::sidebar::{Pos, Sidebar, SidebarPos};
use crate::ir::embed::{EmbedVariables, SectionType};
use crate::ir::pending::{AnchorData, AnchorKind};
use crate::ir::rewriter::RewriterType;
use crate::pass::pure::body::BodyBuilder;
use crate::pass::pure::embed::EmbedBuilder;
use crate::pass::pure::footnote::FootNotesData;
use crate::pass::pure::metadata::MetadataBuilder;
use crate::pass::pure::rewriter::RewriterBuilder;
use crate::pass::pure::sidebar::PureSidebarBuilder;
use crate::util::html::write_token;
use crate::util::html::{Attributes, expect_start};
use crate::util::path::resolve_path;
use crate::util::str::SectionElem;
use anyhow::*;
use html5gum::{StringReader, Tokenizer as HtmlTokenizer};
use std::collections::{HashMap, HashSet};
use std::{path::Path, result::Result::Ok};

use super::tokenizer::{
    BodyTag, Event, EventTokenizer, HeadTag, Label, PeekableTokenizer, Tokenizer,
};
use crate::compile::error::{TypError, TypResult};

mod body;
mod embed;
mod footnote;
mod metadata;
mod rewriter;
mod sidebar;

pub struct PurePass<'a, 'k> {
    pub path: SlugPath,
    pub slug: Key,
    pub cache: Option<&'k Article<'a>>,
    pub config: &'a TypsiteConfig<'a>,
    pub root: &'a Path,
    pub registry: &'k KeyRegistry,
    // metadata
    pub metadata: MetadataBuilder<'a>,
    // error
    error: TypError,
    // article
    schema: Option<&'a Schema>,
    head: Vec<String>,
    body: BodyBuilder<'a>,
    used_rules: HashSet<&'a str>,
    dependency: HashMap<Source, HashSet<UpdatedIndex>>,
    // footnote
    footnotes: FootNotesData,
    // tokenizer
    skip: Option<String>,
    buffer: String,
    content_buffer: Vec<String>, // for heading / metadata
    // heading & sidebar
    heading_level_backtrace: Vec<usize>,
    sidebar: PureSidebarBuilder,
    sidebar_pos: Option<(Pos, usize)>,
    numberings: HashMap<Pos, usize>,
    // anchors
    anchors: HashMap<String, HashMap<AnchorKind, HashSet<usize>>>,
    // rewriter
    rewriter_backtrace: Vec<Option<HashMap<String, String>>>,
}

pub struct PurePassData<'a> {
    pub path: SlugPath,
    pub slug: Key,
    pub config: &'a TypsiteConfig<'a>,
    // article
    pub schema: Option<&'a Schema>,
    pub used_rules: HashSet<&'a str>,
    pub dependency: HashMap<Source, HashSet<UpdatedIndex>>,
    // footnote
    pub footnotes: FootNotesData,
    // metadata
    pub metadata: MetadataBuilder<'a>,
    pub numberings: HashMap<Pos, usize>,
    pub anchors: HashMap<String, HashMap<AnchorKind, HashSet<usize>>>,
    //sidebar
    pub sidebar: PureSidebarBuilder,
}

impl<'a, 'k> PurePassData<'a> {
    fn from(
        pure_pass: PurePass<'a, 'k>,
    ) -> (
        Vec<String>,
        BodyBuilder<'a>,
        TypError,
        Option<&'k Article<'a>>,
        PurePassData<'a>,
    ) {
        let head = pure_pass.head;
        let body = pure_pass.body;
        let error = pure_pass.error;
        let cache = pure_pass.cache;
        let data = PurePassData {
            path: pure_pass.path,
            slug: pure_pass.slug,
            config: pure_pass.config,
            schema: pure_pass.schema,
            used_rules: pure_pass.used_rules,
            dependency: pure_pass.dependency,
            footnotes: pure_pass.footnotes,
            sidebar: pure_pass.sidebar,
            metadata: pure_pass.metadata,
            numberings: pure_pass.numberings,
            anchors: pure_pass.anchors,
        };
        (head, body, error, cache, data)
    }
}

const TITLE_POS: SidebarPos = (vec![], 0);
impl<'a, 'b, 'c, 'k> PurePass<'a, 'k> {
    fn result(&mut self, result: Result<()>) {
        self.error.result(result);
    }

    pub fn run(mut self, tokenizer: HtmlTokenizer<StringReader<'b>>) -> TypResult<Article<'a>> {
        let result = self.visit_html(tokenizer);
        self.result(result);
        let (head, body, mut error, cache, data) = PurePassData::from(self);
        match Self::article(head, body, &mut error, cache, data) {
            Ok(article) if !error.has_error() => return Ok(article),
            Err(err) => error.add(err),
            _ => {}
        }
        Err(error)
    }

    fn article(
        head: Vec<String>,
        mut body: BodyBuilder<'a>,
        error: &mut TypError,
        cache: Option<&'k Article<'a>>,
        data: PurePassData<'a>,
    ) -> Result<Article<'a>> {
        body.build_rewriter_attrs(&data, error);
        let slug = data.slug;
        let path = data.path;
        let schema = data.schema.context("No schema, skip..")?;
        let metadata = data.metadata.build(
            slug.clone(),
            cache.as_ref().map(|it| it.get_meta_contents()),
        )?;
        let (mut full_sidebar, mut embed_sidebar) = data.sidebar.build(&data.config.sidebar);
        let (body_content, body_rewriters, embeds) = body.build(
            &mut |pos| full_sidebar.titles.remove(&pos).unwrap(),
            &mut |pos| embed_sidebar.titles.remove(&pos).unwrap(),
        );
        let body_numberings = data.numberings;
        let full_sidebar_show_children = full_sidebar.show_children;
        let full_sidebar_numberings = full_sidebar.numberings;
        let full_sidebar_anchors = full_sidebar.anchors;
        let embed_show_children = embed_sidebar.show_children;
        let embed_sidebar_numberings = embed_sidebar.numberings;
        let embed_sidebar_anchors = embed_sidebar.anchors;
        let dependency = Dependency::new(data.dependency);
        let used_rules = data.used_rules;
        let body = Body::new(body_content, body_rewriters, body_numberings);
        let full_sidebar = Sidebar::new(
            full_sidebar.contents,
            full_sidebar.titles.remove(&TITLE_POS).unwrap_or_default(),
            full_sidebar_show_children,
            full_sidebar_numberings,
            full_sidebar_anchors,
        );
        let embed_sidebar = Sidebar::new(
            embed_sidebar.contents,
            embed_sidebar.titles.remove(&TITLE_POS).unwrap_or_default(),
            embed_show_children,
            embed_sidebar_numberings,
            embed_sidebar_anchors,
        );
        let anchors = data
            .anchors
            .into_iter()
            .flat_map(|(anchor, kinds)| {
                kinds
                    .into_iter()
                    .map(move |(kind, index)| AnchorData::new(anchor.clone(), kind, index))
            })
            .collect::<Vec<_>>();
        Ok(Article::new(
            slug,
            path,
            metadata,
            schema,
            head,
            body,
            full_sidebar,
            embed_sidebar,
            embeds,
            dependency,
            used_rules,
            anchors,
        ))
    }

    fn visit_html(&mut self, tokenizer: HtmlTokenizer<StringReader<'b>>) -> Result<()> {
        let mut tokenizer = tokenizer.peekable();
        self.visit_tag_block(
            &mut tokenizer,
            "head",
            Tokenizer::<HeadTag>::next,
            Self::handle_head_start_tag,
            Self::handle_head_end_tag,
            Self::push_head_buffer,
        )?;
        self.visit_tag_block(
            &mut tokenizer,
            "body",
            Tokenizer::<BodyTag>::next,
            Self::handle_body_start_tag,
            Self::handle_body_end_tag,
            Self::push_body_buffer,
        )?;
        while let Some(level) = self.heading_level_backtrace.pop() {
            self.push_section_end(level);
        }
        Ok(())
    }

    fn visit_tag_block<T: Label, Next, Start, End, Buffer>(
        &mut self,
        tokenizer: &'c mut PeekableTokenizer<'b>,
        tag: &str,
        mut next: Next,
        mut handle_start: Start,
        mut handle_end: End,
        mut buffer: Buffer,
    ) -> Result<()>
    where
        Next: FnMut(&mut Tokenizer<'b, 'c, T>) -> Option<Result<Event<T>>>,
        Start: FnMut(&mut Self, T) -> Result<()>,
        End: FnMut(&mut Self, T) -> Result<()>,
        Buffer: FnMut(&mut Self),
    {
        self.skip = None;
        expect_start(tokenizer, tag)?;

        let mut pure_tokenizer = Tokenizer::<T>::new(tokenizer);
        let mut end = false;
        while let Some(token) = next(&mut pure_tokenizer) {
            let event = token.context("Error occurred while tokenizing")?;
            let result = match event {
                Event::Eof => {
                    buffer(self);
                    end = true;
                    break;
                }
                Event::End(tag) => {
                    buffer(self);
                    let name = tag.name();
                    if let Some(skip) = &self.skip {
                        if name == skip.as_str() {
                            self.skip = None;
                            handle_end(self, tag)
                        } else {
                            Ok(())
                        }
                    } else {
                        handle_end(self, tag)
                    }
                }
                _ if self.skip.is_some() => Ok(()),
                Event::Start(tag) => {
                    buffer(self);
                    handle_start(self, tag)
                }
                Event::Other(tag) => write_token(&mut self.buffer, &tag),
            };
            self.result(result);
        }
        if !end {
            return Err(anyhow!("Expect a end tag of {tag}"));
        }
        Ok(())
    }

    pub fn new(
        config: &'a TypsiteConfig,
        registry: &'k KeyRegistry,
        path: SlugPath,
        slug: Key,
        cache: Option<&'k Article<'a>>,
    ) -> PurePass<'a, 'k> {
        let metadata = MetadataBuilder::new(slug.clone());
        Self {
            path,
            slug: slug.clone(),
            cache,
            config,
            root: config.typst_path,
            registry,
            head: Vec::new(),
            body: BodyBuilder::new(),
            buffer: String::new(),
            used_rules: HashSet::new(),
            error: TypError::new(slug.clone()),
            skip: None,
            schema: None,
            metadata,
            footnotes: FootNotesData::new(),
            sidebar_pos: None,
            heading_level_backtrace: Vec::new(),
            sidebar: PureSidebarBuilder::new(slug),
            dependency: HashMap::new(),
            content_buffer: Vec::new(),
            numberings: HashMap::new(),
            anchors: HashMap::new(),
            rewriter_backtrace: Vec::new(),
        }
    }

    fn add_anchor(&mut self, kind: AnchorKind, id: String) {
        let anchor = self.config.anchor.get(kind, None, id.as_str());
        self.push_plain(anchor);
        let index = self.body.body_index();
        self.anchors
            .entry(id)
            .or_default()
            .entry(kind)
            .or_default()
            .insert(index);
    }

    fn handle_head_start_tag(&mut self, start_tag: HeadTag) -> Result<()> {
        match start_tag {
            HeadTag::Schema { schema } => {
                let schema = self.config.schemas.get(schema.as_str())?;
                self.schema = Some(schema);
                self.skip = Some("schema".to_string());
            }
            HeadTag::Unique => {
                self.push_head_buffer();
            }
        }
        Ok(())
    }
    fn handle_head_end_tag(&mut self, end_tag: HeadTag) -> std::result::Result<(), Error> {
        match end_tag {
            HeadTag::Unique => {
                self.push_head_buffer();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_body_start_tag(&mut self, tag: BodyTag) -> Result<()> {
        match tag {
            BodyTag::Rewrite { tag, attrs } => {
                let rule = self
                    .config
                    .rules
                    .get(tag.as_str())
                    .with_context(|| format!("No rewrite rule named {tag}"))?;
                let tag_name = self.config.rules.rule_name(&tag).unwrap();
                self.used_rules.insert(tag_name);

                self.push_rewriter_start(tag_name, rule, attrs)?;

                if rule.pass.atom() {
                    self.skip = Some("rewrite".to_string());
                }
            }

            BodyTag::MetaGraph { key, slug } => {
                let slug = self.resolve_slug(&slug, "MetaGraph")?;
                self.metadata.intake_meta_graph(&key, slug);
            }

            BodyTag::MetaOption { key, value } => {
                self.metadata.set_options(key, value);
            }
            BodyTag::MetaContentSet { key } => {
                self.metadata.meta_key = Some(key);
            }
            BodyTag::MetaContentGet { attrs } => {
                if let Some(meta_key) = &self.metadata.meta_key {
                    let inner = attrs.get("get").unwrap();
                    return Err(anyhow!(
                        "[WARN] {}: Metadata CANNOT be embed in other Meta Content: {inner} in {meta_key}, skip",
                        self.slug
                    ));
                } else {
                    let rule = self.config.rules.get("metacontent").unwrap();
                    let tag_name = self.config.rules.rule_name("metacontent").unwrap();
                    self.used_rules.insert(tag_name);
                    self.push_rewriter_start("metacontent", rule, attrs)?;
                }
            }

            BodyTag::Embed {
                slug,
                open,
                variables,
                sidebar,
                heading_level,
            } => {
                self.push_section_ends_if_needed(heading_level);
                self.push_embed(slug, open, variables, sidebar, heading_level)?;
            }

            BodyTag::AnchorGoto { id } => {
                self.add_anchor(AnchorKind::GotoHead, id);
            }
            BodyTag::AnchorDef { id } => {
                self.add_anchor(AnchorKind::Define, id);
            }

            BodyTag::Section { heading_level } => {
                self.push_section_ends_if_needed(heading_level);
                let pos = self.sidebar.intake_heading(heading_level);
                let before_title = self.config.section.before_title();
                self.push_section(heading_level, before_title);
                self.sidebar_pos = Some((pos, 0));
            }
        }
        Ok(())
    }

    fn handle_body_end_tag(&mut self, tag: BodyTag) -> Result<()> {
        match tag {
            BodyTag::Rewrite { tag, .. } => {
                let rule = self
                    .config
                    .rules
                    .get(tag.as_str())
                    .with_context(|| format!("No rewrite rule named {tag}"))?;
                let tag_name = self.config.rules.rule_name(tag.as_str()).unwrap();
                self.push_rewriter_end(tag_name, rule)?;
            }
            BodyTag::MetaContentSet { .. } if self.metadata.meta_key.is_some() => {
                let plain_buffer = std::mem::take(&mut self.content_buffer);
                self.metadata.emit_metacontent_end(plain_buffer);
            }
            BodyTag::AnchorGoto { id } => {
                self.add_anchor(AnchorKind::GotoTail, id.to_string());
            }
            BodyTag::Section { heading_level } => {
                self.sidebar_pos = None;
                self.push_section_start(heading_level);
                let buffer = std::mem::take(&mut self.content_buffer);
                self.sidebar.add_heading_section(heading_level, buffer);
            }
            _ => {}
        }

        Ok(())
    }

    fn push_section_start(&mut self, level: usize) {
        let before_title = self.config.section.before_content();
        self.push_section(level, before_title);
        self.heading_level_backtrace.push(level);
    }
    fn push_section_end(&mut self, level: usize) {
        let after_content = self.config.section.after_content();
        self.push_section(level, after_content);
    }

    fn push_section_ends_if_needed(&mut self, current_level: usize) {
        while let Some(&last_level) = self.heading_level_backtrace.last() {
            if current_level <= last_level {
                self.heading_level_backtrace.pop();
                self.push_section_end(last_level);
            } else {
                break;
            }
        }
    }

    pub fn push_section(&mut self, level: usize, elems: &[SectionElem]) {
        let pos = self.sidebar.current_full_pos();
        for elem in elems {
            match elem {
                SectionElem::Plain(plain) => self.push_plain(plain.to_string()),
                SectionElem::HeadingNumbering => self.push_heading_numbering(pos.clone()),
                SectionElem::Level => self.push_plain(level.to_string()),
                SectionElem::Content | SectionElem::Title => {
                    panic!("Unexpected title/content in before_title in section.html")
                }
            }
        }
    }

    pub fn resolve_slug(&self, slug_path: &str, tag: &str) -> Result<Key> {
        let path = resolve_path(self.root, self.path.parent().unwrap(), slug_path)?;
        let slug = self.config.path_to_slug(path.as_path())?;

        self.registry.slug(slug.as_str()).with_context(|| {
            format!(
                "Failed to resolve slug {slug_path} in {}  ({tag})",
                self.slug
            )
        })
    }

    fn push_meta_or_sidebar_plain(&mut self, text: &str) -> bool {
        if let Some(pos) = &mut self.sidebar_pos {
            self.content_buffer.push(text.to_string());
            pos.1 += 1;
            return false;
        } else if self.metadata.meta_key.is_some() {
            self.content_buffer.push(text.to_string());
            return true;
        }
        false
    }
    fn push_head_buffer(&mut self) {
        if self.buffer.is_empty() {
            return;
        }
        let head = std::mem::take(&mut self.buffer);
        self.head.push(head);
    }

    fn push_body_buffer(&mut self) {
        if self.buffer.is_empty() {
            return;
        }
        let plain = std::mem::take(&mut self.buffer);
        let plain = plain.trim();
        if self.push_meta_or_sidebar_plain(plain) {
            return;
        }
        self.body.push_plain(plain.to_string());
    }

    fn push_heading_numbering(&mut self, pos: Pos) {
        let style = &self.metadata.heading_numbering_style;
        let numbering = style.display(&pos);
        self.push_plain(numbering);
        self.numberings.insert(pos, self.body.body_index());
    }

    fn push_plain(&mut self, plain: String) {
        if self.push_meta_or_sidebar_plain(plain.as_str()) {
            return;
        }
        self.body.push_plain(plain);
    }

    fn push_rewriter<F>(
        &mut self,
        rule: &'a TagRewriteRule,
        attributes: HashMap<String, String>,
        rewriter: F,
    ) -> Result<()>
    where
        F: FnOnce(HashMap<String, String>, Option<(Pos, usize)>, usize) -> RewriterBuilder<'a>,
    {
        let sidebar_pos = self.sidebar_pos.clone();
        let index = if self.metadata.meta_key.is_some() {
            self.content_buffer.len()
        } else {
            self.body.len()
        };
        let dependents = rule.dependents(&attributes, self)?;
        let rewriter = rewriter(attributes, sidebar_pos, index);
        if self.metadata.meta_key.is_some() {
            self.content_buffer.push(String::new());
            self.metadata.push_rewriter(&rewriter);
        } else {
            self.body.push_rewriter(rewriter);
        }
        if let Some(pos) = &mut self.sidebar_pos {
            self.content_buffer.push(String::new());
            pos.1 += 1;
        }
        if let Some(path) = rule.path.as_ref() {
            self.depend_path(Source::Path(path.clone()))
        }
        dependents.into_iter().for_each(|it| self.depend_path(it));
        Ok(())
    }

    fn push_rewriter_start(
        &mut self,
        rule_id: &'a str,
        rule: &'a TagRewriteRule,
        attrs: Attributes,
    ) -> Result<()> {
        let attrs = rule.init(attrs, self);
        let attrs = match attrs {
            Ok(attrs) => {
                self.rewriter_backtrace.push(Some(attrs.clone()));
                attrs
            }
            Err(e) => {
                self.rewriter_backtrace.push(None);
                return Err(e);
            }
        };
        self.push_rewriter(rule, attrs, |attributes, sidebar_pos, index| {
            RewriterBuilder::new(rule_id, RewriterType::Start, attributes, sidebar_pos, index)
        })
    }

    fn push_rewriter_end(&mut self, rule_id: &'a str, rule: &'a TagRewriteRule) -> Result<()> {
        let attrs = self.rewriter_backtrace.pop().unwrap();
        let attrs = match attrs {
            Some(attrs) => attrs,
            None => return Ok(()),
        };
        self.push_rewriter(rule, attrs, |attributes, sidebar_pos, index| {
            RewriterBuilder::new(rule_id, RewriterType::End, attributes, sidebar_pos, index)
        })
    }

    pub fn push_embed(
        &mut self,
        url: String,
        open: bool,
        variables: EmbedVariables,
        sidebar: String,
        heading_level: usize,
    ) -> Result<()> {
        let slug = self.resolve_slug(&url, "Embed");
        let slug = match slug {
            Ok(slug) => slug,
            Err(e) => {
                return Err(e);
            }
        };
        self.metadata.add_children(slug.clone());

        if self.sidebar_pos.is_some() {
            return Err(anyhow!(
                "Embedding in title is not supported, skip embedding {} into {}",
                slug,
                self.slug
            ));
        }

        let body_index = self.body.len();
        let section_type = SectionType::from(sidebar);

        let (full_sidebar_pos, embed_sidebar_pos) = self.sidebar.add_embed_section(heading_level);

        let full_sidebar_pos = (full_sidebar_pos, 0);
        let embed_sidebar_pos = (embed_sidebar_pos, 0);
        self.body.push_embed(EmbedBuilder::new(
            slug.clone(),
            open,
            variables,
            full_sidebar_pos,
            embed_sidebar_pos,
            section_type,
            body_index,
        ));
        self.depend_embed(slug);
        Ok(())
    }

    pub fn add_footnote(&mut self, name: String) -> (String, usize) {
        self.footnotes.add_footnote(name)
    }

    // Record this article depends on which files
    // Also will record the exactly index where the dependency is used
    pub fn depend_path(&mut self, source: Source) {
        if self.metadata.meta_key.is_some() {
            self.depend_metadata(source);
        } else {
            self.depend_body(source);
        }
    }

    fn depend_body(&mut self, source: Source) {
        let index = self.body.rewriter_index();
        self.dependency
            .entry(source)
            .or_default()
            .insert(UpdatedIndex::BodyRewriter(index));
    }

    fn depend_metadata(&mut self, source: Source) {
        let index = self.content_buffer.len() - 1;
        let index = match &self.metadata.meta_key {
            Some(meta_key) => UpdatedIndex::MetaRewriter(meta_key.to_string(), index),
            None => return,
        };
        self.dependency.entry(source).or_default().insert(index);
    }

    fn depend_embed(&mut self, slug: Key) {
        let index = self.body.embed_index();
        self.dependency
            .entry(Source::Article(slug))
            .or_default()
            .insert(UpdatedIndex::Embed(index));
    }
}
