use crate::compile::registry::Key;
use crate::ir::article::data::GlobalData;
use crate::ir::article::dep::Indexes;
use crate::ir::article::sidebar::{Pos, Sidebar, SidebarIndexes};
use crate::ir::embed::Embed;
use crate::ir::pending::{
    BodyNumberingData, EmbedData, Pending, SidebarAnchorData, SidebarData, SidebarIndexesData,
    SidebarNumberingData,
};
use std::collections::HashMap;

pub struct PendingPass<'a, 'b, 'c> {
    slug: Key,
    global_data: &'c GlobalData<'a, 'b, 'c>,
}

impl<'c, 'b: 'c, 'a: 'b> PendingPass<'a, 'b, 'c> {
    pub fn new(slug: Key, global_data: &'c GlobalData<'a, 'b, 'c>) -> Self {
        Self { slug, global_data }
    }

    pub fn run(
        self,
        content: &'c (Vec<String>, Vec<String>, Vec<String>),
        embeds: &[Embed],
        indexes: &Indexes,
    ) -> Pending<'c> {
        match indexes {
            Indexes::All => self.run_self(embeds.iter().collect(), content),
            Indexes::Some(indexes) => {
                let embeds: Vec<&Embed> = indexes.iter().filter_map(|&i| embeds.get(i)).collect();
                self.run_self(embeds, content)
            }
        }
    }

    fn emit_embeds(&self, embeds: Vec<&Embed>) -> Vec<EmbedData<'c>> {
        embeds
            .into_iter()
            .filter_map(|embed| self.emit_embed(embed))
            .collect()
    }

    fn emit_body_numberings(
        &self,
        body_numberings: &HashMap<Pos, usize>,
    ) -> Vec<BodyNumberingData> {
        body_numberings
            .iter()
            .map(|(pos, &body_index)| {
                let pos = pos.clone();
                let anchor = self.slug.to_string();
                BodyNumberingData::new(pos, anchor, body_index)
            })
            .collect()
    }

    fn emit_sidebar_numberings(
        &self,
        sidebar_numberings: &HashMap<Pos, SidebarIndexes>,
    ) -> Vec<SidebarNumberingData> {
        sidebar_numberings
            .iter()
            .map(|(pos, index)| {
                let pos = pos.clone();
                let anchor = self.slug.to_string();
                SidebarNumberingData::new(pos, anchor, index.clone())
            })
            .collect()
    }
    fn emit_sidebar_anchors(
        &self,
        sidebar_anchors: &HashMap<Pos, SidebarIndexes>,
    ) -> Vec<SidebarAnchorData> {
        sidebar_anchors
            .iter()
            .map(|(pos, index)| {
                let pos = pos.clone();
                let anchor = self.slug.to_string();
                SidebarAnchorData::new(pos, anchor, index.clone())
            })
            .collect()
    }
    fn emit_sidebar_indexes(&self, sidebar_show_children: &SidebarIndexes) -> SidebarIndexesData {
        SidebarIndexesData::new(sidebar_show_children.clone())
    }
    fn emit_sidebar(&self, sidebar: &Sidebar) -> SidebarData {
        let indexes = self.emit_sidebar_indexes(sidebar.indexes());
        let numberings = self.emit_sidebar_numberings(sidebar.numberings());
        let anchors = self.emit_sidebar_anchors(sidebar.anchors());
        SidebarData::new(indexes, numberings, anchors)
    }

    fn emit_embed(&self, embed: &Embed) -> Option<EmbedData<'c>> {
        let slug = embed.slug.clone();
        let child = self.global_data.article(slug.as_ref());
        if child.is_none() {
            eprintln!(
                "[WARN] (emit_embed) Embed `{}` not found in {}",
                slug.as_ref(),
                self.slug
            );
            return None;
        }
        let Some(child) = child else {
            return None;
        };
        let child_metadata = child.get_metadata();
        let child_pending = child.get_pending_or_init(self.global_data);
        let section_type = embed.section_type;
        let pos: Pos = embed.sidebar_pos.0.clone();
        let body_index = embed.body_index;
        let full_sidebar_indexes = embed.full_sidebar_indexes.clone();
        let embed_sidebar_indexes = embed.embed_sidebar_indexes.clone();
        let open = embed.open;
        let variables = embed.variables.clone();
        let title = child_metadata.inline(&self.global_data.config.embed.embed_title.body);
        let full_sidebar_title_indexes = child.get_full_sidebar().title_index().clone();
        let embed_sidebar_title_indexes = child.get_embed_sidebar().title_index().clone();
        Some(EmbedData::new(
            pos,
            slug,
            section_type,
            body_index,
            full_sidebar_indexes,
            embed_sidebar_indexes,
            open,
            variables,
            title,
            full_sidebar_title_indexes,
            embed_sidebar_title_indexes,
            child_pending,
        ))
    }

    fn run_self(
        self,
        embeds: Vec<&Embed>,
        content: &'c (Vec<String>, Vec<String>, Vec<String>),
    ) -> Pending<'c> {
        let article = self
            .global_data
            .article(self.slug.as_ref())
            .expect("pending pass should only run for registered articles");
        let style = article.get_meta_options().heading_numbering_style;
        let body_numberings = self.emit_body_numberings(&article.get_body().numberings);
        let full_sidebar = article.get_full_sidebar();
        let embed_sidebar = article.get_embed_sidebar();
        let full_sidebar_data = self.emit_sidebar(full_sidebar);
        let embed_sidebar_data = self.emit_sidebar(embed_sidebar);
        let embeds = self.emit_embeds(embeds);
        let anchors = article.get_anchors();
        Pending::new(
            content,
            style,
            body_numberings,
            full_sidebar_data,
            embed_sidebar_data,
            embeds,
            anchors,
        )
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::compile::registry::{Key, SlugPath};
    use crate::config::TypsiteConfig;
    use crate::ir::article::body::Body;
    use crate::ir::article::dep::{Dependency, Indexes};
    use crate::ir::article::sidebar::{HeadingNumberingStyle, Sidebar, SidebarPos, SidebarType};
    use crate::ir::article::Article;
    use crate::ir::embed::SectionType;
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

    pub(crate) fn run_pending_pass_skips_missing_embed_article_without_panicking() {
        let root = unique_root("pending-pass-missing-embed");
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
            HashMap::from([(slug.clone(), Indexes::All)]),
            HashMap::new(),
        );
        let embeds = vec![Embed::new(
            Key::from("/missing-embed"),
            false,
            Vec::new(),
            SectionType::Full,
            SidebarPos::default(),
            HashSet::new(),
            HashSet::new(),
            0,
        )];
        let content = (vec!["body".to_string()], Vec::new(), Vec::new());

        let pending = PendingPass::new(slug, &global_data).run(&content, &embeds, &Indexes::All);

        assert!(pending.embeds.is_empty());
        assert_eq!(pending.raw.0, vec!["body".to_string()]);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn pending_pass_skips_missing_embed_article_without_panicking() {
        let _guard = crate::test_lock();
        run_pending_pass_skips_missing_embed_article_without_panicking();
    }
}
