mod common;

use common::Project;
use std::collections::HashSet;
use std::path::Path;
use std::process::{Command, Output};
use typsite::compile::typst::{CommandTypst, ExportRequest, TypstRunner};

#[test]
#[ignore = "requires the latest stable Typst executable; CI runs this explicitly"]
fn bundled_authoring_helpers_export_with_latest_typst() {
    let project = Project::named("typsite latest [fixture]");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    typsite::util::fs::copy_dir(
        repository.join("resources/root/lib"),
        project.input.join("lib"),
    )
    .unwrap();
    let packages = project.root.join("packages");
    typsite::util::fs::copy_dir(
        repository.join("packages/typsite"),
        packages.join("local/typsite/0.1.0"),
    )
    .unwrap();
    project.source(
        "latest.typ",
        r#"
#import "/lib/lib.typ": *
#show: schema.with("main")
#title[Latest stable Typst]
#rss(false)
#author[Compatibility test]
= Heading
A paragraph with *emphasis* and a footnote#footnote[Footnote content].
$ x^2 + y^2 = z^2 $
#inline(rect(width: 20pt, height: 10pt))
#inline([SVG footnote #footnote(<frame-note>) and #link("https://example.org")[external]])
#footnote[Frame footnote content] <frame-note>
#inline-math($a/b$)
#inline-math($sum_(i=1)^n i$, block: true)
```rust
fn main() {}
```
"#,
    );
    let runner = CommandTypst::new(std::env::var_os("TYPST").unwrap_or_else(|| "typst".into()))
        .with_package_path(packages.clone());
    let identity = runner
        .identity()
        .expect("Install latest stable Typst or set TYPST");
    eprintln!("Testing {identity}");
    let input = project.input.join("latest.typ");
    let output = project.cache.join("latest.html");
    let fonts = project.config.join("assets/fonts");
    let dependencies = runner
        .export(&ExportRequest {
            root: &project.input,
            input: &input,
            output: &output,
            font_path: &fonts,
        })
        .unwrap();
    let html = std::fs::read_to_string(output).unwrap();
    assert!(html.contains("<schema name=\"main\">"), "{html}");
    assert!(
        html.contains("<metaoption key=\"rss\" value=\"false\">"),
        "{html}"
    );
    assert!(
        html.contains("<svg"),
        "SVG frames must survive latest Typst export"
    );
    assert!(html.contains("Footnote content"));
    assert!(dependencies.iter().any(|path| path.ends_with("latest.typ")));
    assert!(
        dependencies
            .iter()
            .any(|path| path.ends_with("lib/lib.typ"))
    );
    let compiler = project.compiler(std::sync::Arc::new(runner));
    let report = compiler.compile().unwrap();
    assert!(report.is_success(), "{:#?}", report.diagnostics);
    let published = std::fs::read_to_string(project.output.join("latest.html")).unwrap();
    assert!(published.contains("<svg"));
    assert!(published.contains("Footnote content"));
    assert!(published.contains("Frame footnote content"));
    assert!(published.contains("https://example.org"));
    assert!(!published.contains("<rewrite"));
    assert!(!published.contains("<schema"));
    let warm = compiler.compile().unwrap();
    assert!(warm.is_success());
    assert!(!warm.changed());
    assert!(warm.exported.is_empty());
    std::fs::remove_dir_all(packages.join("local/typsite/0.1.0")).unwrap();
    let repaired = common::successful(&compiler);
    assert_eq!(repaired.exported.len(), 1);
    assert!(packages.join("local/typsite/0.1.0/typst.toml").exists());
    let source = std::fs::read_to_string(&input).unwrap();
    for invalid in ["#rss(\"true\")", "#rss(1)"] {
        project.source("latest.typ", &source.replace("#rss(false)", invalid));
        let report = compiler.compile().unwrap();
        assert!(!report.is_success(), "{invalid}");
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("rss expects a bool"))
        );
        assert_eq!(
            std::fs::read_to_string(project.output.join("latest.html")).unwrap(),
            published
        );
    }
    project.source("latest.typ", &source);
    common::successful(&compiler);
}

#[test]
#[ignore = "requires the latest stable Typst executable; CI runs this explicitly"]
fn shipped_templates_support_relationship_edits_with_latest_typst() {
    let project = Project::named("typsite shipped templates [fixture]");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::remove_dir_all(&project.config).unwrap();
    typsite::util::fs::copy_dir(repository.join("resources/.typsite"), &project.config).unwrap();
    typsite::util::fs::copy_dir(
        repository.join("resources/root/lib"),
        project.input.join("lib"),
    )
    .unwrap();
    project.source(
        "index.typ",
        r#"
#import "/lib/lib.typ": *
#show: schema.with("page")
#title[Home]
#author[Smoke test]
= A section
A citation: #cite-title("/other.typ")
== #embed("/child.typ", open: true)
"#,
    );
    project.source(
        "other.typ",
        "#import \"/lib/lib.typ\": *\n#show: schema.with(\"page\")\n#title[Other]\nOther content",
    );
    let child =
        "#import \"/lib/lib.typ\": *\n#show: schema.with(\"page\")\n#title[Child]\nChild content";
    project.source("child.typ", child);
    let runner = CommandTypst::new(std::env::var_os("TYPST").unwrap_or_else(|| "typst".into()))
        .with_package_path(project.root.join("isolated packages"));
    let compiler = project.compiler(std::sync::Arc::new(runner));
    common::successful(&compiler);
    let first = std::fs::read_to_string(project.output.join("index.html")).unwrap();
    assert!(first.contains("Child content"));
    assert!(first.contains("Other"));
    assert!(!first.contains("<embed"));
    assert!(!first.contains("<rewrite"));
    project.source("child.typ", &child.replace("Child", "Updated child"));
    assert_eq!(common::successful(&compiler).exported.len(), 1);
    assert!(
        std::fs::read_to_string(project.output.join("index.html"))
            .unwrap()
            .contains("Updated child content")
    );
    common::clean_equivalent(&project, &compiler);
}

fn demo_command(project: &Project) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_typsite"));
    command
        .current_dir(&project.root)
        .env("TYPST_PACKAGE_PATH", project.root.join("isolated packages"))
        .env(
            "TYPST_PACKAGE_CACHE_PATH",
            project.root.join("isolated package cache"),
        )
        .env("SOURCE_DATE_EPOCH", "1772496000")
        .env("TYPST_IGNORE_SYSTEM_FONTS", "true")
        .env_remove("TYPST_IGNORE_EMBEDDED_FONTS")
        .env_remove("TYPST_FONT_PATHS");
    command
}

fn demo_success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn check_demo_links(project: &Project, page: &Path, html: &str) {
    let tags: Vec<_> = html5gum::Tokenizer::new(html)
        .map(Result::unwrap)
        .filter_map(|token| match token {
            html5gum::Token::StartTag(tag) => Some(tag),
            _ => None,
        })
        .collect();
    let ids: HashSet<_> = tags
        .iter()
        .flat_map(|tag| &tag.attributes)
        .filter(|(name, _)| name.as_slice() == b"id")
        .map(|(_, value)| value.as_slice())
        .collect();
    for tag in &tags {
        for (name, value) in &tag.attributes {
            if !matches!(name.as_slice(), b"href" | b"src" | b"xlink:href") {
                continue;
            }
            let url = std::str::from_utf8(value).unwrap();
            if let Some(fragment) = url
                .strip_prefix('#')
                .filter(|fragment| !fragment.is_empty())
            {
                assert!(
                    ids.contains(fragment.as_bytes()),
                    "{} links to missing local anchor {url}",
                    page.display()
                );
            }
            if url.is_empty()
                || url.starts_with(['#', '?'])
                || url.starts_with("//")
                || url.contains(':')
            {
                continue;
            }
            let path = url.split(['#', '?']).next().unwrap();
            let destination = if path.starts_with('/') {
                project.output.join(path.trim_start_matches('/'))
            } else {
                project.output.join(page.parent().unwrap()).join(path)
            };
            let mut with_html = destination.as_os_str().to_owned();
            with_html.push(".html");
            assert!(
                destination.is_file()
                    || Path::new(&with_html).is_file()
                    || destination.join("index.html").is_file(),
                "{} links to missing local URL {url}",
                page.display()
            );
        }
    }
}

fn check_process_diagram(project: &Project, page: &str) {
    let html = std::fs::read_to_string(project.output.join(page)).unwrap();
    assert!(
        html.contains("href=\"/process.css\""),
        "{page}: missing diagram styling"
    );
    let mut in_diagram = false;
    let mut diagrams = 0;
    for token in html5gum::Tokenizer::new(&html).map(Result::unwrap) {
        match token {
            html5gum::Token::StartTag(tag)
                if tag.name == b"div"
                    && tag
                        .attributes
                        .get(b"class".as_slice())
                        .is_some_and(|class| class.as_slice() == b"process-diagram") =>
            {
                assert!(!in_diagram, "{page}: nested process diagrams");
                assert_eq!(
                    tag.attributes.get(b"role".as_slice()).unwrap().as_slice(),
                    b"img"
                );
                assert!(
                    !tag.attributes
                        .get(b"aria-label".as_slice())
                        .unwrap()
                        .is_empty()
                );
                in_diagram = true;
            }
            html5gum::Token::StartTag(tag) if tag.name == b"svg" && in_diagram => {
                let value =
                    |name: &[u8]| std::str::from_utf8(tag.attributes.get(name).unwrap()).unwrap();
                assert_eq!(value(b"width"), "660pt", "{page}");
                assert_eq!(value(b"height"), "660pt", "{page}");
                assert_eq!(value(b"viewbox"), "0 0 660 660", "{page}");
                diagrams += 1;
            }
            html5gum::Token::EndTag(tag) if tag.name == b"div" => in_diagram = false,
            _ => {}
        }
    }
    assert!(diagrams > 0, "{page}: missing square process diagram");
}

#[test]
#[ignore = "requires the latest stable Typst executable; CI runs this explicitly"]
fn initialized_demo_builds_all_pages_and_migration_with_latest_typst() {
    let mut project = Project::named("typsite full demo [fixture]");
    std::fs::remove_dir_all(&project.config).unwrap();
    std::fs::remove_dir_all(&project.input).unwrap();
    project.input = project.root.join("root");
    project.cache = project.root.join(".cache");
    demo_success(demo_command(&project).arg("init").output().unwrap());
    let options = std::fs::read_to_string(project.config.join("options.toml"))
        .unwrap()
        .replace("base_url = \"\"", "base_url = \"https://example.org\"");
    project.configure("options.toml", &options);

    let executable = std::env::var_os("TYPST").unwrap_or_else(|| "typst".into());
    let executable = Path::new(&executable);
    let executable = if executable
        .parent()
        .is_some_and(|path| !path.as_os_str().is_empty())
    {
        std::path::absolute(executable).unwrap()
    } else {
        executable.to_owned()
    };
    let compile = || {
        demo_success(
            demo_command(&project)
                .args(["compile", "--typst"])
                .arg(&executable)
                .output()
                .unwrap(),
        )
    };
    let cold = compile();
    eprintln!("{cold}");
    let pages: Vec<_> = walkdir::WalkDir::new(&project.input)
        .into_iter()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "typ"))
        .filter(|path| !path.starts_with(project.input.join("lib")))
        .collect();
    assert!(
        cold.contains(&format!("{} exported", pages.len())),
        "{cold}"
    );
    for source in &pages {
        let destination = source
            .strip_prefix(&project.input)
            .unwrap()
            .with_extension("html");
        let html = std::fs::read_to_string(project.output.join(&destination))
            .unwrap_or_else(|error| panic!("{}: {error}", destination.display()));
        assert!(html.contains("</html>"), "{}", destination.display());
        check_demo_links(&project, &destination, &html);
        for unresolved in [
            "<schema",
            "<rewrite",
            "<embed",
            "<metacontent",
            "<metagraph",
            "<metaoption",
            "<metadata",
            "<anchorgoto",
            "<anchordef",
        ] {
            assert!(
                !html.contains(unresolved),
                "{}: {unresolved}",
                destination.display()
            );
        }
    }
    for path in ["migrate-to-018.html", "en/migrate-to-018.html"] {
        let html = std::fs::read_to_string(project.output.join(path)).unwrap();
        assert!(html.contains("0.1.8"), "{path}");
        assert!(html.contains(".typsite-outputs.json"), "{path}");
    }
    for path in ["index.html", "en/index.html"] {
        let html = std::fs::read_to_string(project.output.join(path)).unwrap();
        assert!(html.contains("migrate-to-018"), "{path}");
        assert!(html.contains("<svg"), "{path}");
        let icons: Vec<_> = html5gum::Tokenizer::new(&html)
            .map(Result::unwrap)
            .filter_map(|token| match token {
                html5gum::Token::StartTag(tag)
                    if tag.name == b"img"
                        && tag
                            .attributes
                            .get(b"alt".as_slice())
                            .is_some_and(|alt| alt.as_slice() == b"Typsite") =>
                {
                    Some(tag)
                }
                _ => None,
            })
            .collect();
        assert_eq!(icons.len(), 1, "{path}: one shared, responsive icon");
        let attributes = &icons[0].attributes;
        for (name, value) in [("src", "/icon.png"), ("width", "192"), ("height", "192")] {
            assert_eq!(
                attributes
                    .get(name.as_bytes())
                    .map(|value| value.as_slice()),
                Some(value.as_bytes()),
                "{path}: {name}"
            );
        }
        let style = std::str::from_utf8(attributes.get(b"style".as_slice()).unwrap()).unwrap();
        for rule in ["width: 192px", "max-width: 100%", "height: auto"] {
            assert!(style.contains(rule), "{path}: {style}");
        }
        assert!(
            !html.contains("data:image/png;base64,"),
            "{path}: do not frame the large PNG as SVG"
        );
    }
    for path in [
        "index.html",
        "en/index.html",
        "migrate-to-018.html",
        "en/migrate-to-018.html",
    ] {
        let html = std::fs::read_to_string(project.output.join(path)).unwrap();
        let text = html5gum::Tokenizer::new(&html)
            .map(Result::unwrap)
            .filter_map(|token| match token {
                html5gum::Token::String(value) => Some(String::from_utf8(value.to_vec()).unwrap()),
                _ => None,
            })
            .collect::<String>();
        for instruction in [
            "[site]",
            "base_url",
            "rss_path",
            "rss_limit",
            "rss_content",
            "summary",
            "full",
            "[default_metadata.options]",
            "false",
            "#rss(true)",
            "#embed(",
            "publish/rss.xml",
            "http://localhost:8080/rss.xml",
        ] {
            assert!(
                text.contains(instruction),
                "missing RSS instruction {instruction} in {path}"
            );
        }
    }
    for page in [
        "index.html",
        "en/index.html",
        "example.html",
        "en/example.html",
    ] {
        check_process_diagram(&project, page);
    }
    let diagram_css = std::fs::read_to_string(project.output.join("process.css")).unwrap();
    for rule in ["overflow-x: auto", "aspect-ratio: 1", "height: auto"] {
        assert!(
            diagram_css.contains(rule),
            "missing responsive diagram rule {rule}"
        );
    }
    assert!(project.output.join("fonts/Inria Sans.ttf").exists());
    assert!(project.output.join("icon.png").exists());
    assert!(
        !project.root.join("isolated package cache/preview").exists(),
        "the demo must not download preview packages"
    );
    let feed_path = project.output.join("rss.xml");
    let summary_feed = std::fs::read_to_string(&feed_path).unwrap();
    let selected = ["/article", "/typst", "/en/article", "/en/typst"];
    assert_eq!(summary_feed.matches("<item>").count(), selected.len());
    for slug in selected {
        assert!(
            summary_feed.contains(&format!("<link>https://example.org{slug}</link>")),
            "missing {slug}"
        );
    }
    for excluded in ["/index", "/en/index", "/migrate-to-018", "/en/example"] {
        assert!(
            !summary_feed.contains(&format!("<link>https://example.org{excluded}</link>")),
            "unexpected {excluded}"
        );
    }
    assert!(summary_feed.contains("application/rss+xml"));
    assert!(!summary_feed.contains("<content:encoded>"));
    assert!(summary_feed.contains("+0000</pubDate>"));
    for source in &pages {
        let page = project.output.join(
            source
                .strip_prefix(&project.input)
                .unwrap()
                .with_extension("html"),
        );
        assert!(
            std::fs::read_to_string(page)
                .unwrap()
                .contains("type=\"application/rss+xml\"")
        );
    }
    project.configure(
        "options.toml",
        &options.replace("rss_content = \"summary\"", "rss_content = \"full\""),
    );
    let full = compile();
    assert!(full.contains("0 exported"), "{full}");
    let full_feed = std::fs::read_to_string(&feed_path).unwrap();
    assert_eq!(
        full_feed.matches("<content:encoded>").count(),
        selected.len()
    );
    assert!(full_feed.contains("&lt;section"));
    assert!(full_feed.contains("https://example.org/"));
    let mut reader = quick_xml::Reader::from_str(&full_feed);
    while !matches!(reader.read_event().unwrap(), quick_xml::events::Event::Eof) {}
    let published = project.snapshot();
    let warm = compile();
    assert!(
        warm.contains("0 exported, 0 rendered, 0 written, 0 removed"),
        "{warm}"
    );
    assert_eq!(published, project.snapshot());

    let source = std::fs::read_to_string(project.input.join("en/typst.typ")).unwrap();
    project.source(
        "en/typst.typ",
        &format!("{source}\nAn incremental demo edit.\n"),
    );
    let edited = compile();
    assert!(edited.contains("1 exported"), "{edited}");
    assert!(
        std::fs::read_to_string(project.output.join("en/typst.html"))
            .unwrap()
            .contains("An incremental demo edit.")
    );
    assert!(
        std::fs::read_to_string(&feed_path)
            .unwrap()
            .contains("An incremental demo edit.")
    );
    let incremental = project.snapshot();
    demo_success(demo_command(&project).arg("clean").output().unwrap());
    compile();
    assert_eq!(incremental, project.snapshot());

    // Execute the copyable migration examples, not just the surrounding prose.
    // Both use real demo articles and must keep the feed deduplicated.
    for path in ["migrate-to-018.typ", "en/migrate-to-018.typ"] {
        let guide = std::fs::read_to_string(project.input.join(path)).unwrap();
        let rss_section = &guide[guide.find("= RSS").unwrap()..];
        let site_options = rss_section
            .split_once("```toml\n")
            .unwrap()
            .1
            .split_once("```")
            .unwrap()
            .0;
        project.configure(
            "options.toml",
            &format!(
                "{}\n{site_options}",
                options.split("[site]").next().unwrap()
            ),
        );
        let snippet = rss_section
            .split_once("```typ\n")
            .unwrap()
            .1
            .split_once("```")
            .unwrap()
            .0;
        project.source("feed.typ", snippet);
        let report = compile();
        assert!(report.contains("1 exported"), "{path}: {report}");
        let feed = std::fs::read_to_string(&feed_path).unwrap();
        assert_eq!(feed.matches("<item>").count(), selected.len(), "{path}");
        assert!(
            !feed.contains("<content:encoded>"),
            "the documented configuration uses summary mode"
        );
    }
}
