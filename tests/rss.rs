mod common;

use common::{FakeTypst, Project, clean_equivalent, html, successful};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;
use typsite::compile::options::CompileOptions;

#[derive(Debug, Default)]
struct Xml {
    name: String,
    attributes: BTreeMap<String, String>,
    text: String,
    children: Vec<Xml>,
}

impl Xml {
    fn parse(source: &str) -> Self {
        let mut reader = Reader::from_str(source);
        let mut stack = vec![Self::default()];
        loop {
            match reader.read_event().unwrap() {
                Event::Start(tag) => stack.push(Self::from_tag(&tag)),
                Event::Empty(tag) => stack
                    .last_mut()
                    .unwrap()
                    .children
                    .push(Self::from_tag(&tag)),
                Event::End(tag) => {
                    let node = stack.pop().unwrap();
                    assert_eq!(node.name.as_bytes(), tag.name().as_ref());
                    stack.last_mut().unwrap().children.push(node);
                }
                Event::Text(text) => stack
                    .last_mut()
                    .unwrap()
                    .text
                    .push_str(&text.unescape().unwrap()),
                Event::Decl(_) | Event::Comment(_) => {}
                Event::Eof => break,
                event => panic!("unexpected XML event {event:?}"),
            }
        }
        assert_eq!(stack.len(), 1);
        let mut document = stack.pop().unwrap();
        assert_eq!(document.children.len(), 1);
        document.children.pop().unwrap()
    }

    fn from_tag(tag: &quick_xml::events::BytesStart<'_>) -> Self {
        Self {
            name: std::str::from_utf8(tag.name().as_ref()).unwrap().into(),
            attributes: tag
                .attributes()
                .map(|attribute| {
                    let attribute = attribute.unwrap();
                    (
                        std::str::from_utf8(attribute.key.as_ref()).unwrap().into(),
                        attribute.unescape_value().unwrap().into_owned(),
                    )
                })
                .collect(),
            ..Self::default()
        }
    }

    fn child(&self, name: &str) -> &Self {
        self.children
            .iter()
            .find(|child| child.name == name)
            .unwrap_or_else(|| panic!("missing {name} in {self:?}"))
    }

    fn items(&self) -> Vec<&Self> {
        self.child("channel")
            .children
            .iter()
            .filter(|child| child.name == "item")
            .collect()
    }
}

fn configure(project: &Project, site: &str) {
    let options = fs::read_to_string(project.config.join("options.toml")).unwrap();
    let prefix = options.split("[site]").next().unwrap();
    project.configure("options.toml", &format!("{prefix}[site]\n{site}\n"));
}

fn article(title: &str, date: &str, description: &str, body: &str) -> String {
    html(&format!(
        "<metacontent set=\"title\">{title}</metacontent><metacontent set=\"date\">{date}</metacontent><metacontent set=\"summary\">{description}</metacontent>{body}"
    ))
}

fn rss(project: &Project, path: &str) -> Xml {
    let xml = Xml::parse(&fs::read_to_string(project.output.join(path)).unwrap());
    assert_eq!(xml.name, "rss");
    assert_eq!(xml.attributes["version"], "2.0");
    xml
}

#[test]
fn summary_feed_has_valid_xml_rss_dates_metadata_and_discovery() {
    let project = Project::new();
    configure(
        &project,
        r#"
base_url = "https://example.org/blog/"
title = "A & B"
description = "News <em>from</em> us"
rss_path = "/feeds/news.xml"
"#,
    );
    project.source(
        "first.typ",
        &article(
            "<span>Fish &amp; Chips</span>",
            "2026-03-02 18:25",
            "<p>A &amp; B</p><p>Next</p>",
            "<p>Not in summary feed.</p>",
        ),
    );
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    let feed = rss(&project, "feeds/news.xml");
    let channel = feed.child("channel");
    assert_eq!(channel.child("title").text, "A & B");
    assert_eq!(channel.child("description").text, "News from us");
    assert_eq!(channel.child("link").text, "https://example.org/blog/");
    assert_eq!(
        channel.child("atom:link").attributes["href"],
        "https://example.org/blog/feeds/news.xml"
    );
    assert_eq!(
        channel.child("lastBuildDate").text,
        "Mon, 02 Mar 2026 18:25:00 +0000"
    );
    let items = feed.items();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].child("title").text, "Fish & Chips");
    assert_eq!(items[0].child("description").text, "A & B Next");
    assert_eq!(
        items[0].child("pubDate").text,
        "Mon, 02 Mar 2026 18:25:00 +0000"
    );
    assert_eq!(
        items[0].child("guid").text,
        "https://example.org/blog/first"
    );
    assert_eq!(items[0].child("guid").attributes["isPermaLink"], "true");
    assert!(
        !items[0]
            .children
            .iter()
            .any(|child| child.name == "content:encoded")
    );
    let page = fs::read_to_string(project.output.join("first.html")).unwrap();
    assert!(page.contains("type=\"application/rss+xml\""));
    assert!(page.contains("https://example.org/blog/feeds/news.xml"));
    assert_eq!(page.matches("rel=\"alternate\"").count(), 1);
    let sitemap = Xml::parse(&fs::read_to_string(project.output.join("sitemap.xml")).unwrap());
    assert_eq!(sitemap.child("url").child("lastmod").text, "2026-03-02");
    assert!(!successful(&compiler).changed());
    clean_equivalent(&project, &compiler);
}

#[test]
fn full_feed_contains_composed_content_not_site_chrome_and_absolutizes_urls() {
    let project = Project::new();
    configure(
        &project,
        "base_url = \"https://example.org\"\nrss_content = \"full\"",
    );
    project.configure("schemas/main.html", "<head><script>preview_script</script></head><body><nav>site navigation</nav><content/><footer/></body>");
    project.source("posts/a.typ", &article("A", "2026-03-02", "Summary", "<h1 id=\"section\">Heading</h1><p>Body &amp; text ]]&gt;</p><a href=\"#section\">Jump</a><a href=\"../other?x=1&amp;y=2\">Other</a><img src=\"images/a.png\"><img src=\"/image.png\"><script>not_in_feed</script><embed slug=\"/child\"></embed>"));
    project.source(
        "child.typ",
        &article("Child", "2026-03-01", "", "<p>Embedded body</p>"),
    );
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    let feed = rss(&project, "rss.xml");
    let body = &feed.items()[0].child("content:encoded").text;
    for expected in [
        "Heading",
        "Embedded body",
        "Body &amp; text ]]&gt;",
        "href=\"https://example.org/posts/a#section\"",
        "href=\"https://example.org/other?x=1&amp;y=2\"",
        "src=\"https://example.org/posts/images/a.png\"",
        "src=\"https://example.org/image.png\"",
    ] {
        assert!(body.contains(expected), "missing {expected} in {body}");
    }
    for unwanted in [
        "site navigation",
        "preview_script",
        "not_in_feed",
        "<html",
        "<footer",
        "<embed",
        "<rewrite",
    ] {
        assert!(!body.contains(unwanted), "{unwanted}: {body}");
    }
    assert_eq!(feed.items()[0].child("description").text, "Summary");
    clean_equivalent(&project, &compiler);
}

#[test]
fn dates_sort_by_full_timestamp_validate_calendar_and_respect_limits() {
    let project = Project::new();
    configure(
        &project,
        "base_url = \"https://example.org\"\nrss_limit = 3",
    );
    for (path, date) in [
        ("a.typ", "2026-03-02 09:00"),
        ("b.typ", "2026-03-02T08:30:00-01:00"),
        ("c.typ", "Mon, 02 Mar 2026 10:00:00 +0000"),
        ("d.typ", "2026-02-30"),
        ("e.typ", "Unknown Date"),
    ] {
        project.source(path, &article(path, date, "", "Body"));
    }
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    let feed = rss(&project, "rss.xml");
    let items = feed.items();
    assert_eq!(
        items
            .iter()
            .map(|item| item.child("title").text.as_str())
            .collect::<Vec<_>>(),
        ["c.typ", "b.typ", "a.typ"]
    );
    assert_eq!(
        items[1].child("pubDate").text,
        "Mon, 02 Mar 2026 09:30:00 +0000"
    );
    configure(&project, "base_url = \"https://example.org\"");
    successful(&compiler);
    let feed = rss(&project, "rss.xml");
    assert!(
        feed.items()[3..]
            .iter()
            .all(|item| !item.children.iter().any(|child| child.name == "pubDate"))
    );
    let sitemap = Xml::parse(&fs::read_to_string(project.output.join("sitemap.xml")).unwrap());
    assert!(
        sitemap.children[3..]
            .iter()
            .all(|item| !item.children.iter().any(|child| child.name == "lastmod"))
    );
    project.source(
        "a.typ",
        &article(
            "a.typ",
            "2026-03-01",
            "",
            "<metacontent set=\"updated\">2026-03-02 11:00:01</metacontent>",
        ),
    );
    successful(&compiler);
    let feed = rss(&project, "rss.xml");
    assert_eq!(feed.items()[0].child("title").text, "a.typ");
    assert_eq!(
        feed.items()[0].child("pubDate").text,
        "Sun, 01 Mar 2026 00:00:00 +0000"
    );
    assert_eq!(
        feed.child("channel").child("lastBuildDate").text,
        "Mon, 02 Mar 2026 11:00:01 +0000"
    );
}

#[test]
fn urls_encode_literal_slugs_and_nonpretty_urls_and_empty_sites_work() {
    let project = Project::new();
    configure(&project, "base_url = \"https://example.org/sub path\"");
    let compiler = project.compiler_with_options(
        Arc::new(FakeTypst::default()),
        CompileOptions {
            pretty_url: false,
            ..CompileOptions::default()
        },
    );
    successful(&compiler);
    assert!(rss(&project, "rss.xml").items().is_empty());
    project.source("汉字 & page.typ", &article("A", "2024-02-29", "", "Body"));
    successful(&compiler);
    let feed = rss(&project, "rss.xml");
    assert_eq!(
        feed.items()[0].child("link").text,
        "https://example.org/sub%20path/%E6%B1%89%E5%AD%97%20&%20page.html"
    );
    assert_eq!(
        feed.items()[0].child("pubDate").text,
        "Thu, 29 Feb 2024 00:00:00 +0000"
    );
    clean_equivalent(&project, &compiler);
}

#[test]
fn config_changes_switch_modes_move_disable_and_reenable_without_typst_exports() {
    let project = Project::new();
    project.source(
        "a.typ",
        &article("A", "2026-03-02", "Summary", "<p>Full body</p>"),
    );
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    assert!(!project.output.join("rss.xml").exists());
    configure(&project, "base_url = \"https://example.org\"");
    assert!(successful(&compiler).exported.is_empty());
    assert!(project.output.join("rss.xml").exists());
    configure(
        &project,
        "base_url = \"https://example.org\"\nrss_content = \"full\"\nrss_path = \"feeds/all.xml\"",
    );
    let report = successful(&compiler);
    assert!(report.exported.is_empty());
    assert!(report.removed.contains(&"rss.xml".into()));
    assert!(
        rss(&project, "feeds/all.xml").items()[0]
            .child("content:encoded")
            .text
            .contains("Full body")
    );
    for disabled in ["rss_path = \"\"", "rss_limit = 0", "base_url = \"\""] {
        let site = if disabled.starts_with("base_url") {
            disabled.to_owned()
        } else {
            format!("base_url = \"https://example.org\"\n{disabled}")
        };
        configure(&project, &site);
        assert!(successful(&compiler).exported.is_empty());
        assert!(!project.output.join("feeds/all.xml").exists());
        assert!(!project.output.join("rss.xml").exists());
        assert!(
            !fs::read_to_string(project.output.join("a.html"))
                .unwrap()
                .contains("application/rss+xml")
        );
    }
    configure(&project, "base_url = \"https://example.org\"");
    successful(&compiler);
    assert!(project.output.join("rss.xml").exists());
    fs::remove_file(project.input.join("a.typ")).unwrap();
    successful(&compiler);
    assert!(rss(&project, "rss.xml").items().is_empty());
    clean_equivalent(&project, &compiler);
}

#[test]
fn full_feeds_follow_embed_edits_recover_from_failed_exports_and_drop_deleted_items() {
    let project = Project::new();
    configure(
        &project,
        "base_url = \"https://example.org\"\nrss_content = \"full\"",
    );
    project.source(
        "parent.typ",
        &article(
            "Parent",
            "2026-03-02",
            "",
            "<embed slug=\"/child\"></embed>",
        ),
    );
    project.source(
        "child.typ",
        &article("Child", "2026-03-01", "", "<p>Original content</p>"),
    );
    let runner = Arc::new(FakeTypst::default());
    let compiler = project.compiler(runner.clone());
    successful(&compiler);
    let before = project.snapshot();
    project.source(
        "child.typ",
        &article("Child", "2026-03-01", "", "<p>Updated content</p>"),
    );
    runner
        .fail_next
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(!compiler.compile().unwrap().is_success());
    assert_eq!(before, project.snapshot());
    assert_eq!(successful(&compiler).exported.len(), 1);
    let feed = rss(&project, "rss.xml");
    assert!(feed.items().iter().all(|item| {
        item.child("content:encoded")
            .text
            .contains("Updated content")
    }));
    clean_equivalent(&project, &compiler);
    project.source(
        "parent.typ",
        &article("Parent", "2026-03-02", "", "<p>No embed</p>"),
    );
    fs::remove_file(project.input.join("child.typ")).unwrap();
    successful(&compiler);
    let feed = rss(&project, "rss.xml");
    assert_eq!(feed.items().len(), 1);
    assert!(
        !feed.items()[0]
            .child("content:encoded")
            .text
            .contains("Updated content")
    );
    clean_equivalent(&project, &compiler);
}

#[test]
fn invalid_feed_configuration_and_collisions_preserve_last_good_outputs() {
    let project = Project::new();
    project.source("a.typ", &article("A", "2026-03-02", "", "Body"));
    configure(&project, "base_url = \"https://example.org\"");
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    let good = project.snapshot();
    for invalid in [
        "base_url = \"example.org\"",
        "base_url = \"file:///tmp/site\"",
        "base_url = \"https://example.org?query\"",
        "base_url = \"https://user:pass@example.org\"",
        "base_url = \"https://example.org#fragment\"",
        "base_url = \"https://example.org\"\nrss_content = \"invalid\"",
        "base_url = \"https://example.org\"\nrss_path = \"../escape.xml\"",
        "base_url = \"https://example.org\"\nrss_path = \"/\"",
    ] {
        configure(&project, invalid);
        assert!(compiler.compile().is_err(), "{invalid}");
        assert_eq!(project.snapshot(), good);
    }
    configure(&project, "base_url = \"https://example.org\"");
    project.source("rss.xml", "manual feed collision");
    assert!(compiler.compile().is_err());
    assert_eq!(project.snapshot(), good);
    fs::remove_file(project.input.join("rss.xml")).unwrap();
    assert!(!successful(&compiler).changed());
}
