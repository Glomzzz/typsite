use crate::compile::options::ProjectOptions;
use crate::compile::registry::Key;
use crate::ir::article::sidebar::{HeadingNumberingStyle, SidebarType};
use crate::ir::metadata::Metadata;
use crate::ir::metadata::content::{MetaContent, MetaContents};
use crate::ir::metadata::graph::MetaNode;
use crate::ir::metadata::options::MetaOptions;
use crate::ir::rewriter::MetaRewriter;
use crate::pass::pure::rewriter::RewriterBuilder;
use anyhow::anyhow;
use std::collections::{HashMap, HashSet};

pub struct MetadataBuilder<'a> {
    slug: Key,
    options: &'a ProjectOptions,
    pub(crate) meta_key: Option<String>,
    meta_rewriter_buffer: Vec<MetaRewriter<'a>>,
    // content supported metadata
    contents: HashMap<String, MetaContent<'a>>,
    pub(crate) heading_numbering_style: HeadingNumberingStyle,
    pub(crate) sidebar_type: SidebarType,
    // slugs
    parent: Option<Key>,
    cited: HashSet<Key>,
    children: HashSet<Key>,
    errors: Vec<anyhow::Error>,
}

impl<'a> MetadataBuilder<'a> {
    pub fn new(self_slug: Key, project_options: &'a ProjectOptions) -> Self {
        let options = &project_options.project;

        let heading_numbering_style = options.default_metadata.options.heading_numbering;
        let sidebar_type = options.default_metadata.options.sidebar_type;
        let parent = None;
        // options
        // .default_metadata
        // .graph
        // .parent
        // .as_ref()
        // .and_then(|parent| {
        //     let parent = config.format_slug(parent);
        //     registry
        //         .know(parent, "default_metadata.graph.parent", "options.toml")
        //         .ok()
        // })
        // .filter(|parent| parent.as_str() != self_slug.as_str());

        Self {
            slug: self_slug,
            options: project_options,
            meta_key: None,
            meta_rewriter_buffer: Vec::new(),
            contents: HashMap::new(),
            heading_numbering_style,
            sidebar_type,
            parent,
            cited: HashSet::new(),
            children: HashSet::new(),
            errors: Vec::new(),
        }
    }

    pub fn intake_meta_graph(&mut self, kind: &str, slug: Key) {
        if self.slug == slug && kind.eq_ignore_ascii_case("parent") {
            self.errors.push(anyhow!(
                "An article cannot be its own parent: {}",
                self.slug
            ));
            return;
        }
        match kind.to_lowercase().as_str() {
            "parent" => {
                self.parent = Some(slug);
            }
            "cite" => {
                self.cited.insert(slug);
            }
            "child" => {
                self.children.insert(slug);
            }
            _ => {
                eprintln!("[WARN] MetadataBuilder: Unknown metadata graph kind: {kind}");
            }
        }
    }

    pub fn set_options(&mut self, key: String, value: String) {
        match key.as_str() {
            "heading_numbering" | "heading-numbering" => {
                self.heading_numbering_style = HeadingNumberingStyle::from(value.as_ref());
            }
            "sidebar" => {
                self.sidebar_type = SidebarType::from(value.as_ref());
            }
            _ => {
                eprintln!("[WARN] Unknown metadata option: {key}");
            }
        }
    }

    pub fn push_rewriter(&mut self, builder: &RewriterBuilder<'a>) {
        self.meta_rewriter_buffer.push(builder.build_meta());
    }

    pub fn emit_metacontent_end(&mut self, content: Vec<String>) {
        let Some(metadata_key) = self.meta_key.take() else {
            self.errors.push(anyhow!(
                "Unbalanced metacontent close for {}: closing a metadata content block without an open key",
                self.slug
            ));
            self.meta_rewriter_buffer.clear();
            return;
        };
        let atom_buffer = std::mem::take(&mut self.meta_rewriter_buffer);
        let content = MetaContent::new(content, atom_buffer);
        self.contents.insert(metadata_key, content);
    }

    pub fn add_cite(&mut self, slug: Key) {
        self.cited.insert(slug);
    }

    pub fn add_children(&mut self, slug: Key) {
        self.children.insert(slug);
    }

    pub(crate) fn build(
        mut self,
        slug: Key,
        cache: Option<&MetaContents<'a>>,
    ) -> anyhow::Result<Metadata<'a>> {
        if !self.errors.is_empty() {
            let details = self
                .errors
                .drain(..)
                .map(|error| error.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(anyhow!(details));
        }
        let updated = cache
            .as_ref()
            .map(|cache| !cache.same_contents(&self.contents))
            .unwrap_or(true);
        let contents = MetaContents::new(slug.clone(), self.contents, updated, self.options);
        let options = MetaOptions {
            heading_numbering_style: self.heading_numbering_style,
            sidebar_type: self.sidebar_type,
        };
        let node = MetaNode {
            slug,
            parent: self.parent,
            parents: HashSet::new(),
            backlinks: HashSet::new(),
            references: self.cited,
            children: self.children,
        };
        Ok(Metadata {
            contents,
            options,
            node,
        })
    }
}
