use crate::compile::registry::Key;
use crate::compile::{compile_options, proj_options};
use crate::ir::article::Article;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct GeneratedFile {
    pub path: PathBuf,
    pub content: String,
}

pub struct GeneratedSite {
    pub files: Vec<GeneratedFile>,
    pub removed: Vec<PathBuf>,
}

struct PageEntry {
    url: String,
    title: String,
    description: Option<String>,
    date: Option<String>,
    date_ymd: Option<(i32, u32, u32)>,
}

pub fn generate_site_outputs(articles: &HashMap<Key, Article<'_>>) -> Result<GeneratedSite> {
    let options = proj_options()?;
    let site = &options.site;
    let rss_path = normalize_output_path(&site.rss_path);
    let sitemap_path = normalize_output_path(&site.sitemap_path);
    let base_url = normalize_base_url(&site.base_url);

    if base_url.is_none() {
        let mut removed = Vec::new();
        if let Some(path) = rss_path {
            removed.push(path);
        }
        if let Some(path) = sitemap_path {
            removed.push(path);
        }
        return Ok(GeneratedSite {
            files: Vec::new(),
            removed,
        });
    }

    let base_url = base_url.unwrap();
    let pretty_url = compile_options()?.pretty_url;
    let mut entries = collect_entries(articles, &base_url, pretty_url);

    let mut files = Vec::new();
    let mut removed = Vec::new();

    if let Some(path) = sitemap_path {
        let sitemap = build_sitemap(&mut entries);
        files.push(GeneratedFile {
            path,
            content: sitemap,
        });
    }

    if let Some(path) = rss_path {
        if site.rss_limit == 0 {
            removed.push(path);
        } else {
            let rss = build_rss(&mut entries, &base_url, site);
            files.push(GeneratedFile { path, content: rss });
        }
    }

    Ok(GeneratedSite { files, removed })
}

fn collect_entries(
    articles: &HashMap<Key, Article<'_>>,
    base_url: &str,
    pretty_url: bool,
) -> Vec<PageEntry> {
    articles
        .values()
        .map(|article| {
            let slug = article.slug.as_ref();
            let path = if pretty_url {
                slug.to_string()
            } else {
                format!("{slug}.html")
            };
            let url = format!("{base_url}{path}");
            let contents = article.get_meta_contents();
            let title = contents
                .get("page-title")
                .or_else(|| contents.get("title"))
                .map(|it| it.to_string())
                .filter(|it| !it.trim().is_empty())
                .unwrap_or_else(|| slug.to_string());
            let description = first_meta_value(contents, &["description", "summary", "abstract"]);
            let date = first_meta_value(contents, &["updated", "date", "lastmod"])
                .filter(|it| !is_unknown_date(it));
            let date_ymd = date.as_deref().and_then(parse_ymd);
            PageEntry {
                url,
                title,
                description,
                date,
                date_ymd,
            }
        })
        .collect()
}

fn build_sitemap(entries: &mut [PageEntry]) -> String {
    entries.sort_by(|a, b| a.url.cmp(&b.url));
    let mut sitemap = String::new();
    sitemap.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    sitemap.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
    for entry in entries.iter() {
        sitemap.push_str("  <url>\n");
        sitemap.push_str(&format!("    <loc>{}</loc>\n", xml_escape(&entry.url)));
        if let Some((y, m, d)) = entry.date_ymd {
            sitemap.push_str(&format!("    <lastmod>{y:04}-{m:02}-{d:02}</lastmod>\n"));
        }
        sitemap.push_str("  </url>\n");
    }
    sitemap.push_str("</urlset>\n");
    sitemap
}

fn build_rss(
    entries: &mut [PageEntry],
    base_url: &str,
    site: &crate::compile::options::SiteOptions,
) -> String {
    entries.sort_by(|a, b| match (a.date_ymd, b.date_ymd) {
        (Some(a_date), Some(b_date)) => b_date.cmp(&a_date),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.title.cmp(&b.title),
    });

    let limit = site.rss_limit.min(entries.len());
    let channel_title = normalize_text(&site.title).unwrap_or_else(|| base_url.to_string());
    let channel_description =
        normalize_text(&site.description).unwrap_or_else(|| "RSS Feed".to_string());
    let last_build = entries.iter().find_map(|it| it.date_ymd.map(rfc3339_ymd));

    let mut rss = String::new();
    rss.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    rss.push_str("<rss version=\"2.0\">\n");
    rss.push_str("  <channel>\n");
    rss.push_str(&format!(
        "    <title>{}</title>\n",
        xml_escape(&channel_title)
    ));
    rss.push_str(&format!("    <link>{}</link>\n", xml_escape(base_url)));
    rss.push_str(&format!(
        "    <description>{}</description>\n",
        xml_escape(&channel_description)
    ));
    if let Some(last_build) = last_build {
        rss.push_str(&format!(
            "    <lastBuildDate>{}</lastBuildDate>\n",
            last_build
        ));
    }

    for entry in entries.iter().take(limit) {
        rss.push_str("    <item>\n");
        rss.push_str(&format!(
            "      <title>{}</title>\n",
            xml_escape(&entry.title)
        ));
        rss.push_str(&format!("      <link>{}</link>\n", xml_escape(&entry.url)));
        rss.push_str(&format!("      <guid>{}</guid>\n", xml_escape(&entry.url)));
        if let Some(date) = entry.date_ymd.map(rfc3339_ymd) {
            rss.push_str(&format!("      <pubDate>{}</pubDate>\n", date));
        } else if let Some(date) = entry.date.as_ref() {
            rss.push_str(&format!("      <pubDate>{}</pubDate>\n", xml_escape(date)));
        }
        if let Some(description) = entry.description.as_ref() {
            rss.push_str(&format!(
                "      <description>{}</description>\n",
                xml_escape(description)
            ));
        }
        rss.push_str("    </item>\n");
    }

    rss.push_str("  </channel>\n");
    rss.push_str("</rss>\n");
    rss
}

fn normalize_base_url(base_url: &str) -> Option<String> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.trim_end_matches('/').to_string())
    }
}

fn normalize_output_path(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed.trim_start_matches('/')))
    }
}

fn normalize_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn first_meta_value(
    contents: &crate::ir::metadata::content::MetaContents<'_>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| contents.get(key))
        .map(|it| it.to_string())
        .and_then(|it| normalize_text(&it))
}

fn is_unknown_date(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("unknown date")
}

fn parse_ymd(value: &str) -> Option<(i32, u32, u32)> {
    let value = value.trim();
    let mut parts = value.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some((year, month, day))
}

fn rfc3339_ymd((year, month, day): (i32, u32, u32)) -> String {
    format!("{year:04}-{month:02}-{day:02}T00:00:00Z")
}

fn xml_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}
