//! Deterministic RSS 2.0 and sitemap publication from the current rendered site.

use super::options::{ProjectOptions, RssContent, SiteOptions};
use super::project::validate_relative;
use super::registry::Key;
use super::render::RenderedPage;
use crate::ir::article::Article;
use crate::ir::metadata::content::MetaContents;
use anyhow::{Context, Result, ensure};
use html5gum::{Token, Tokenizer};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use time::format_description::well_known::{Rfc2822, Rfc3339};
use time::{Date, Month, OffsetDateTime, Time, UtcOffset};
use url::Url;

pub struct GeneratedFile {
    pub path: PathBuf,
    pub content: String,
}

struct PageEntry {
    url: Url,
    title: String,
    description: Option<String>,
    published: Option<OffsetDateTime>,
    modified: Option<OffsetDateTime>,
    body: Option<String>,
    rss: bool,
}

/// Validate once, before exporting or publishing. A blank base URL disables both
/// outputs; blank paths disable the corresponding output, and limit=0 disables RSS.
pub struct SiteOutputs<'a> {
    site: &'a SiteOptions,
    base: Option<Url>,
    rss: Option<(PathBuf, Url)>,
    sitemap: Option<PathBuf>,
    pretty_url: bool,
}

impl<'a> SiteOutputs<'a> {
    pub fn new(options: &'a ProjectOptions) -> Result<Self> {
        let site = &options.project.site;
        let base = if site.base_url.trim().is_empty() {
            None
        } else {
            let mut base = Url::parse(site.base_url.trim())
                .context("site.base_url must be an absolute HTTP(S) URL")?;
            ensure!(
                matches!(base.scheme(), "http" | "https")
                    && base.host_str().is_some()
                    && base.username().is_empty()
                    && base.password().is_none()
                    && base.query().is_none()
                    && base.fragment().is_none(),
                "site.base_url must be an absolute HTTP(S) URL without credentials, query, or fragment"
            );
            // Treat an optional deployment prefix as a directory, not a filename.
            if !base.path().ends_with('/') {
                let path = format!("{}/", base.path());
                base.set_path(&path);
            }
            Some(base)
        };
        let mut outputs = Self {
            site,
            base,
            rss: None,
            sitemap: None,
            pretty_url: options.compile.pretty_url,
        };
        if let Some(base) = &outputs.base {
            if site.rss_limit != 0
                && let Some(path) = output_path(&site.rss_path)?
            {
                let url = output_url(base, &path)?;
                outputs.rss = Some((path, url));
            }
            outputs.sitemap = output_path(&site.sitemap_path)?;
        }
        Ok(outputs)
    }

    pub fn discovery_link(&self) -> Option<String> {
        self.rss.as_ref().zip(self.base.as_ref()).map(|((_, url), base)| {
            format!(
                "<link rel=\"alternate\" type=\"application/rss+xml\" title=\"{}\" href=\"{}\">",
                xml_escape(&channel_title(self.site, base)),
                xml_escape(url.as_str())
            )
        })
    }

    pub fn generate(
        &self,
        articles: &HashMap<Key, Article<'_>>,
        pages: &[RenderedPage],
    ) -> Result<Vec<GeneratedFile>> {
        let Some(base) = &self.base else {
            return Ok(Vec::new());
        };
        if self.rss.is_none() && self.sitemap.is_none() {
            return Ok(Vec::new());
        }
        // RSS containers select only actual direct embeds. A selected container
        // stays excluded even when another container embeds it. Sitemap selection
        // is independent and continues to include every compiled article.
        let selected: HashSet<_> = articles
            .values()
            .filter(|article| article.get_meta_options().rss)
            .flat_map(Article::direct_embeds)
            .collect();
        let pages: HashMap<_, _> = pages.iter().map(|page| (&page.slug, &page.body)).collect();
        let mut entries = Vec::new();
        for article in articles.values() {
            let slug = article.slug.as_ref();
            let path = if self.pretty_url {
                slug.trim_start_matches('/').to_string()
            } else {
                format!("{}.html", slug.trim_start_matches('/'))
            };
            let url = output_url(base, &PathBuf::from(path))?;
            let contents = article.get_meta_contents();
            let title = first_meta_value(contents, &["page-title", "title"])
                .unwrap_or_else(|| slug.to_string());
            let description = first_meta_value(contents, &["description", "summary", "abstract"]);
            let published = first_date(contents, &["date"]);
            let modified = first_date(contents, &["updated", "lastmod", "date"]);
            let rss = selected.contains(&article.slug) && !article.get_meta_options().rss;
            let body = if rss && self.rss.is_some() && self.site.rss_content == RssContent::Full {
                let body = pages
                    .get(&article.slug)
                    .with_context(|| format!("Missing rendered RSS body for {slug}"))?;
                Some(feed_html(body, &url)?)
            } else {
                None
            };
            entries.push(PageEntry {
                url,
                title,
                description,
                published,
                modified,
                body,
                rss,
            });
        }
        let mut files = Vec::new();
        if let Some(path) = &self.sitemap {
            files.push(GeneratedFile {
                path: path.clone(),
                content: build_sitemap(&mut entries),
            });
        }
        if let Some((path, url)) = &self.rss {
            entries.retain(|entry| entry.rss);
            files.push(GeneratedFile {
                path: path.clone(),
                content: build_rss(&mut entries, base, url, self.site),
            });
        }
        Ok(files)
    }
}

fn build_sitemap(entries: &mut [PageEntry]) -> String {
    entries.sort_by(|a, b| a.url.as_str().cmp(b.url.as_str()));
    let mut sitemap = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for entry in entries {
        sitemap.push_str("  <url>\n");
        element(&mut sitemap, "loc", entry.url.as_str(), "    ");
        if let Some(date) = entry.modified {
            element(&mut sitemap, "lastmod", &date.date().to_string(), "    ");
        }
        sitemap.push_str("  </url>\n");
    }
    sitemap.push_str("</urlset>\n");
    sitemap
}

fn build_rss(entries: &mut [PageEntry], base: &Url, feed: &Url, site: &SiteOptions) -> String {
    entries.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.url.as_str().cmp(b.url.as_str()))
    });
    let mut rss = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\" xmlns:content=\"http://purl.org/rss/1.0/modules/content/\">\n  <channel>\n",
    );
    element(&mut rss, "title", &channel_title(site, base), "    ");
    element(&mut rss, "link", base.as_str(), "    ");
    element(
        &mut rss,
        "description",
        &plain_text(&site.description)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "RSS Feed".into()),
        "    ",
    );
    rss.push_str(&format!(
        "    <atom:link href=\"{}\" rel=\"self\" type=\"application/rss+xml\"/>\n",
        xml_escape(feed.as_str())
    ));
    if let Some(date) = entries.iter().filter_map(|entry| entry.modified).max() {
        element(&mut rss, "lastBuildDate", &rss_date(date), "    ");
    }
    for entry in entries.iter().take(site.rss_limit) {
        rss.push_str("    <item>\n");
        element(&mut rss, "title", &entry.title, "      ");
        element(&mut rss, "link", entry.url.as_str(), "      ");
        rss.push_str(&format!(
            "      <guid isPermaLink=\"true\">{}</guid>\n",
            xml_escape(entry.url.as_str())
        ));
        if let Some(date) = entry.published.or(entry.modified) {
            element(&mut rss, "pubDate", &rss_date(date), "      ");
        }
        if let Some(description) = &entry.description {
            element(&mut rss, "description", description, "      ");
        }
        if let Some(body) = &entry.body {
            // XML-escaped HTML is equivalent to CDATA and safe even for `]]>`.
            element(&mut rss, "content:encoded", body, "      ");
        }
        rss.push_str("    </item>\n");
    }
    rss.push_str("  </channel>\n</rss>\n");
    rss
}

fn channel_title(site: &SiteOptions, base: &Url) -> String {
    plain_text(&site.title)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| base.to_string())
}

fn output_path(path: &str) -> Result<Option<PathBuf>> {
    let path = path.trim();
    if path.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(path.trim_start_matches('/'));
    validate_relative(&path)?;
    Ok(Some(path))
}

/// Append literal project path components, percent-encoding spaces, Unicode,
/// `#`, and `?` rather than interpreting filenames as URL syntax.
fn output_url(base: &Url, path: &std::path::Path) -> Result<Url> {
    let mut url = base.clone();
    let mut segments = url
        .path_segments_mut()
        .map_err(|_| anyhow::anyhow!("Invalid site base URL"))?;
    segments.pop_if_empty();
    for part in path.components() {
        let std::path::Component::Normal(part) = part else {
            anyhow::bail!("Invalid feed URL path: {}", path.display());
        };
        segments.push(part.to_str().context("Feed path must be Unicode")?);
    }
    drop(segments);
    Ok(url)
}

fn first_meta_value(contents: &MetaContents<'_>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| contents.get(key))
        .filter_map(|value| plain_text(&value))
        .find(|value| !value.is_empty())
}

fn first_date(contents: &MetaContents<'_>, keys: &[&str]) -> Option<OffsetDateTime> {
    keys.iter()
        .filter_map(|key| contents.get(key))
        .filter_map(|value| plain_text(&value))
        .find_map(|value| parse_date(&value))
}

/// Typst metadata is HTML. Strip markup and decode its entities before putting
/// titles/dates/summaries into XML, rather than displaying literal `<span>` tags.
fn plain_text(html: &str) -> Option<String> {
    let mut text = String::new();
    let mut hidden = false;
    for token in Tokenizer::new(html) {
        match token.ok()? {
            Token::StartTag(tag) if matches!(tag.name.as_slice(), b"script" | b"style") => {
                hidden = true
            }
            Token::EndTag(tag) if matches!(tag.name.as_slice(), b"script" | b"style") => {
                hidden = false
            }
            Token::String(value) if !hidden => text.push_str(std::str::from_utf8(&value).ok()?),
            Token::StartTag(tag)
                if matches!(tag.name.as_slice(), b"br" | b"p" | b"div" | b"li") =>
            {
                text.push(' ')
            }
            Token::EndTag(tag) if matches!(tag.name.as_slice(), b"p" | b"div" | b"li") => {
                text.push(' ')
            }
            _ => {}
        }
    }
    Some(text.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn parse_date(value: &str) -> Option<OffsetDateTime> {
    let value = value.trim();
    let zoned =
        OffsetDateTime::parse(value, &Rfc3339).or_else(|_| OffsetDateTime::parse(value, &Rfc2822));
    if let Ok(date) = zoned {
        let date = date.to_offset(UtcOffset::UTC);
        return (1900..=9999).contains(&date.year()).then_some(date);
    }
    let (date, clock) = value
        .split_once([' ', 'T'])
        .map_or((value, None), |(date, clock)| (date, Some(clock)));
    let parts: Vec<_> = date.split('-').collect();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return None;
    }
    let year = parts[0].parse().ok()?;
    if !(1900..=9999).contains(&year) {
        return None;
    }
    let date = Date::from_calendar_date(
        year,
        Month::try_from(parts[1].parse::<u8>().ok()?).ok()?,
        parts[2].parse().ok()?,
    )
    .ok()?;
    let clock = if let Some(clock) = clock {
        let parts: Vec<_> = clock.split(':').collect();
        if !(2..=3).contains(&parts.len()) || parts.iter().any(|part| part.len() != 2) {
            return None;
        }
        Time::from_hms(
            parts[0].parse().ok()?,
            parts[1].parse().ok()?,
            parts.get(2).map_or(Some(0), |part| part.parse().ok())?,
        )
        .ok()?
    } else {
        Time::MIDNIGHT
    };
    Some(date.with_time(clock).assume_utc())
}

fn rss_date(date: OffsetDateTime) -> String {
    // parse_date bounds the year to RFC 2822's supported range and uses UTC.
    date.format(&Rfc2822).expect("validated RSS date")
}

/// A feed contains the composed article, not the site's schema/navigation/footer.
/// Make ordinary links/images absolute so they work outside the original page.
fn feed_html(html: &str, article_url: &Url) -> Result<String> {
    let mut output = String::new();
    let mut script = false;
    let mut svg_depth: usize = 0;
    for token in Tokenizer::new(html) {
        let token = token.map_err(|error| anyhow::anyhow!("Invalid RSS HTML: {error:?}"))?;
        match token {
            Token::StartTag(tag) if tag.name == b"script" => script = true,
            Token::EndTag(tag) if tag.name == b"script" => script = false,
            _ if script => {}
            Token::StartTag(tag) => {
                if tag.name == b"svg" && !tag.self_closing {
                    svg_depth += 1;
                }
                output.push('<');
                output.push_str(std::str::from_utf8(&tag.name)?);
                for (name, value) in tag.attributes {
                    let name = std::str::from_utf8(&name)?;
                    let value = std::str::from_utf8(&value)?;
                    // SVG glyph/gradient references must keep pointing inside
                    // this SVG, not at an external HTML document's fragment.
                    let svg_resource = svg_depth != 0 && tag.name != b"a" && value.starts_with('#');
                    let value = if matches!(name, "href" | "src" | "poster" | "xlink:href")
                        && !value.trim().is_empty()
                        && !svg_resource
                    {
                        article_url
                            .join(value)
                            .with_context(|| format!("Invalid RSS URL {value}"))?
                            .to_string()
                    } else {
                        value.to_owned()
                    };
                    output.push_str(&format!(" {name}=\"{}\"", xml_escape(&value)));
                }
                output.push_str(if tag.self_closing { "/>" } else { ">" });
            }
            Token::EndTag(tag) => {
                if tag.name == b"svg" {
                    svg_depth = svg_depth.saturating_sub(1);
                }
                output.push_str(&format!("</{}>", std::str::from_utf8(&tag.name)?));
            }
            Token::String(value) => output.push_str(&xml_escape(std::str::from_utf8(&value)?)),
            _ => {}
        }
    }
    Ok(output)
}

fn element(xml: &mut String, name: &str, value: &str, indent: &str) {
    xml.push_str(&format!("{indent}<{name}>{}</{name}>\n", xml_escape(value)));
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
            '\t'
            | '\n'
            | '\r'
            | '\u{20}'..='\u{d7ff}'
            | '\u{e000}'..='\u{fffd}'
            | '\u{10000}'..='\u{10ffff}' => escaped.push(ch),
            _ => escaped.push('\u{fffd}'),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_validate_leap_days_times_and_offsets() {
        for date in [
            "2023-02-29",
            "2024-04-31",
            "2024-02-00",
            "2024-13-01",
            "2024-01-01 24:00",
            "2024-01-01 00:60",
            "2024-01-01 01:00:60",
            "2024-01-01 garbage",
            "Unknown Date",
            "1899-12-31",
            "2024-1-1",
        ] {
            assert!(parse_date(date).is_none(), "{date}");
        }
        assert_eq!(
            rss_date(parse_date("2024-02-29").unwrap()),
            "Thu, 29 Feb 2024 00:00:00 +0000"
        );
        assert_eq!(
            rss_date(parse_date("2026-03-02T01:00:00+02:00").unwrap()),
            "Sun, 01 Mar 2026 23:00:00 +0000"
        );
        assert_eq!(
            rss_date(parse_date("2026-03-02 18:25:59").unwrap()),
            "Mon, 02 Mar 2026 18:25:59 +0000"
        );
    }

    #[test]
    fn metadata_text_decodes_entities_and_skips_markup_and_scripts() {
        assert_eq!(
            plain_text("<p>A &amp; B</p><p><em>Next</em></p><script>hidden</script>"),
            Some("A & B Next".into())
        );
        assert_eq!(
            xml_escape("x\u{1} & < > \" '"),
            "x\u{fffd} &amp; &lt; &gt; &quot; &apos;"
        );
    }

    #[test]
    fn feed_html_preserves_escaping_and_svg_resources_without_site_scripts() {
        let base = Url::parse("https://example.org/posts/a").unwrap();
        let html = feed_html("<p>Literal &lt;tag&gt; &amp; ]]&gt;</p><a href=\"../b?x=1&amp;y=2\" title=\"&quot;&amp;\">B</a><svg><defs><path id=\"glyph\"/></defs><use xlink:href=\"#glyph\"/><a href=\"#note\">note</a></svg><script>hidden</script>", &base).unwrap();
        assert!(html.contains("Literal &lt;tag&gt; &amp; ]]&gt;"), "{html}");
        assert!(
            html.contains("href=\"https://example.org/b?x=1&amp;y=2\""),
            "{html}"
        );
        assert!(html.contains("title=\"&quot;&amp;\""), "{html}");
        assert!(html.contains("xlink:href=\"#glyph\""), "{html}");
        assert!(
            html.contains("href=\"https://example.org/posts/a#note\""),
            "{html}"
        );
        assert!(!html.contains("hidden"));
    }

    #[test]
    fn output_urls_encode_filenames_instead_of_interpreting_url_syntax() {
        let base = Url::parse("https://example.org/blog/").unwrap();
        assert_eq!(
            output_url(&base, std::path::Path::new("a#b?c.xml"))
                .unwrap()
                .as_str(),
            "https://example.org/blog/a%23b%3Fc.xml"
        );
    }
}
