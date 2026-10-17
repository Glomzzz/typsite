//! Whole-page rendering of an already resolved site. No rendered fragments are
//! loaded from disk, and every lazy cache lives only for this build.

use super::graph::ResolvedSite;
use super::report::{BuildReport, Stage};
use crate::compile::error::TypResult;
use crate::config::TypsiteConfig;
use crate::ir::article::data::GlobalData;
use crate::ir::article::dep::Indexes;
use crate::ir::article::sidebar::SidebarType;
use crate::ir::embed::SectionType;
use crate::pass::pass_schema;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

pub struct RenderedPage {
    pub path: PathBuf,
    pub slug: super::registry::Key,
    pub html: String,
    /// Composed article content, without the page schema, sidebar, or footer.
    pub body: String,
}

pub fn render<'a>(
    config: &'a TypsiteConfig<'a>,
    site: &ResolvedSite<'a>,
    report: &mut BuildReport,
    feed_link: Option<&str>,
) -> Vec<RenderedPage> {
    let articles = &site.articles;
    let global = GlobalData::new(
        config,
        articles,
        articles
            .keys()
            .cloned()
            .map(|slug| (slug, OnceLock::new()))
            .collect(),
        articles
            .keys()
            .cloned()
            .map(|slug| (slug, Indexes::All))
            .collect(),
        articles
            .keys()
            .cloned()
            .map(|slug| (slug, Indexes::All))
            .collect(),
    );
    for slug in &site.metadata_order {
        let article = &articles[slug];
        let contents = article.get_meta_contents();
        let mut keys: Vec<_> = contents.keys().into_iter().collect();
        keys.sort_unstable();
        for key in keys {
            contents.pass_content(key, Indexes::All, &global);
        }
        contents.init_parent(&global);
    }
    let empty = Vec::new();
    // Barrier: all references/backlinks must exist before a page schema can
    // consume them. This permits mutual citations without recursive rendering.
    let bodies: Vec<_> = site
        .metadata_order
        .par_iter()
        .map(|slug| -> TypResult<_> {
            let article = &articles[slug];
            let pending = article.get_pending_or_init(&global)?;
            let (body, full, embedded) = pending.based_on(
                config,
                &global,
                Some(&empty),
                Some(&empty),
                article.get_meta_options().sidebar_type,
                SectionType::Full,
            );
            let body = body.join("");
            let sidebar = if article.get_meta_options().sidebar_type == SidebarType::All {
                full
            } else {
                embedded
            }
            .join("");
            if !article.get_meta_node().backlinks.is_empty() {
                global.init_reference(article, &body, &sidebar)?;
            }
            if !article.get_meta_node().references.is_empty() {
                global.init_backlink(article, &body, &sidebar)?;
            }
            Ok((article, body, sidebar))
        })
        .collect();
    let mut prepared = HashMap::new();
    for result in bodies {
        match result {
            Ok((article, body, sidebar)) => {
                prepared.insert(article.slug.clone(), (article, body, sidebar));
            }
            Err(error) => report.error(Stage::Render, None, error),
        }
    }
    if !report.is_success() {
        return Vec::new();
    }
    let rendered: Vec<_> = site
        .metadata_order
        .par_iter()
        .map(|slug| {
            let (article, body, sidebar) = &prepared[slug];
            pass_schema(config, article.schema, article, body, sidebar, &global).map(|mut html| {
                if let Some(link) = feed_link {
                    html.head.end(link.to_owned());
                }
                RenderedPage {
                    path: article.path.to_path_buf(),
                    slug: article.slug.clone(),
                    html: html.to_html(),
                    body: body.clone(),
                }
            })
        })
        .collect();
    let mut pages = Vec::new();
    for result in rendered {
        match result {
            Ok(page) => pages.push(page),
            Err(error) => report.error(Stage::Render, None, error),
        }
    }
    for error in global.take_runtime_errors() {
        report.error(Stage::Render, None, error);
    }
    report.rendered = pages.len();
    pages
}
