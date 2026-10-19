//! Real, offline native-Typst book tests. CI runs these alongside typst_latest.
mod common;

use anyhow::Result;
use common::Project;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use typsite::compile::pdf::{PdfCompiler, PdfOptions};
use typsite::compile::typst::{CommandTypst, ExportRequest, PdfRequest, TypstRunner};

struct CheckedTypst {
    real: CommandTypst,
    assertions: String,
    calls: AtomicUsize,
}

impl TypstRunner for CheckedTypst {
    fn identity(&self) -> Result<String> {
        Ok(format!(
            "{}|PDF assertions: {}",
            self.real.identity()?,
            self.assertions
        ))
    }
    fn prepare(&self, packages: Option<&Path>) -> Result<Vec<PathBuf>> {
        self.real.prepare(packages)
    }
    fn export(&self, _: &ExportRequest<'_>) -> Result<Vec<PathBuf>> {
        anyhow::bail!("Native books must not export HTML")
    }
    fn pdf(&self, request: &PdfRequest<'_>) -> Result<Vec<PathBuf>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        // Keep byte comparisons portable without changing process environment.
        let driver = format!(
            "#set document(date: datetime(year: 1980, month: 1, day: 1))\n{}\n{}",
            request.driver, self.assertions
        );
        self.real.pdf(&PdfRequest {
            driver: &driver,
            ..*request
        })
    }
}

fn setup(assertions: &str) -> (Project, Arc<CheckedTypst>, PathBuf) {
    let project = Project::named("typsite native PDF [fixture]");
    typsite::util::fs::copy_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/root/lib"),
        project.input.join("lib"),
    )
    .unwrap();
    let runner = Arc::new(CheckedTypst {
        real: CommandTypst::new(std::env::var_os("TYPST").unwrap_or_else(|| "typst".into()))
            .with_package_path(project.root.join("package namespace")),
        assertions: assertions.into(),
        calls: AtomicUsize::new(0),
    });
    let output = project.root.join("books/site.pdf");
    (project, runner, output)
}

fn compiler(
    project: &Project,
    runner: Arc<CheckedTypst>,
    options: PdfOptions,
    output: &Path,
) -> PdfCompiler {
    PdfCompiler::with_runner(
        project.input.clone(),
        project.config.clone(),
        project.cache.clone(),
        output.to_owned(),
        None,
        options,
        runner,
    )
    .unwrap()
}

const LIB: &str = "#import \"/lib/lib.typ\": *\n#show: schema.with(\"page\")\n";

#[test]
#[ignore = "requires latest stable Typst; CI runs this explicitly"]
fn native_book_preserves_order_occurrences_metadata_links_and_numbering() {
    let (project, runner, output) = setup(
        r#"
#context {
  let facts(name) = query(metadata).filter(it => type(it.value) == dictionary and it.value.at("typsite-pdf", default: none) == name)
  let articles = facts("article")
  assert(articles.map(it => it.value.scope.source) == ("/index", "/chapters/child [v1]", "/shared", "/shared", "/appendix"), message: "Article order")
  let titles = query(heading).filter(it => it.at("label", default: none) == label("typsite-pdf-title"))
  assert(titles.map(it => it.level) == (1, 2, 3, 2, 1), message: "Rebased heading levels: " + repr(titles.map(it => it.level)))
  assert(query(heading).len() == 7, message: "No phantom heading wrappers")
  assert(counter(heading).at(titles.at(1).location()) == (1, 2), message: "No phantom counter increments")
  assert(counter(heading).at(titles.at(3).location()) == (1, 3), message: "Repeated article numbering")
  let anchors = facts("anchor-record").filter(it => it.value.scope.source == "/shared" and it.value.id == "local")
  assert(anchors.len() == 2 and anchors.first().value.scope.occurrence != anchors.last().value.scope.occurrence)
  let destinations = anchors.map(it => it.location())
  assert(query(link).filter(it => destinations.contains(it.dest)).len() == 2, message: "Occurrence-local links")
  assert(query(metadata).filter(it => it.value == "root-author-used").len() == 1, message: "Root occurrence metadata")
  assert(query(metadata).filter(it => it.value == "child-author-used").len() == 1, message: "Child occurrence metadata")
  assert(query(metadata).filter(it => it.value == "summary-retrieved").len() == 2, message: "Metadata is not printed until retrieved")
  assert(query(math.equation).len() >= 3, message: "Math remains native")
  assert(not query(link).any(it => it.body == [Eq.]), message: "Custom supplements must not replace the native reference number")
  assert(query(image).len() == 1, message: "Images retain original source-relative paths")
}
"#,
    );
    project.source("lib/value.typ", "#let value = [Version one]");
    project.source(
        "index.typ",
        &format!(
            r#"{LIB}
#set text(font: "Inria Sans")
#import "/lib/value.typ": value
#title[Root]
#author[Root author#metadata("root-author-used")]
#metacontent("summary")[Summary#metadata("summary-retrieved")]
Before. #value. #get-metacontent("summary"). See #cite-title("./appendix.typ").
= Local section <section>
#link(<section>)[Local section link].
#set math.equation(numbering: "(1)")
$ x^2 = 1 $ <equation>
See @equation and @equation[Eq.]. A footnote#footnote[Native footnote].
#let child = "./chapters/child [v1].typ"
= #embed(child)
After child.
#embed("./shared.html", open: false, heading-level: "peer")
After repeated article.
"#
        ),
    );
    project.source(
        "chapters/child [v1].typ",
        &format!(
            r#"{LIB}
#set text(size: 10pt)
#title[Child $x^2$]
#author[Child author#metadata("child-author-used")]
#parent("/index.typ")
#get-metacontent("summary", from: "parent")
#image("relative.svg", width: 12pt)
= #embed("../shared.typ")
"#
        ),
    );
    project.source("chapters/relative.svg", r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8"/></svg>"#);
    project.source(
        "shared.typ",
        &format!(
            r#"{LIB}
#import "@local/typsite:0.1.0": anchor, goto
#title[Shared]
Shared body. #get-metacontent("author").
#anchor("local") #goto("local")[Local anchor]. Native math $a/b$.
"#
        ),
    );
    project.source(
        "appendix.typ",
        &format!("{LIB}\n#title[Appendix]\nAppendix body."),
    );
    let options = PdfOptions {
        appendices: vec!["appendix.typ".into()],
        ..PdfOptions::default()
    };
    let compiler = compiler(&project, runner.clone(), options, &output);
    let cold = compiler.compile().unwrap();
    assert!(cold.compiled && cold.written);
    assert!(fs::read(&output).unwrap().starts_with(b"%PDF-"));
    assert!(!compiler.compile().unwrap().compiled);
    let original = fs::read(&output).unwrap();
    project.source("lib/value.typ", "#let value = [Version two]");
    assert!(compiler.compile().unwrap().compiled);
    let incremental = fs::read(&output).unwrap();
    assert_ne!(original, incremental);
    fs::remove_dir_all(&project.cache).unwrap();
    compiler.compile().unwrap();
    assert_eq!(
        incremental,
        fs::read(&output).unwrap(),
        "Incremental book must equal clean output"
    );
}

#[test]
#[ignore = "requires latest stable Typst; CI runs this explicitly"]
fn deep_native_forest_does_not_depend_on_layout_state_propagation() {
    let (project, runner, output) = setup(
        r#"
#context {
  let articles = query(metadata).filter(it => type(it.value) == dictionary and it.value.at("typsite-pdf", default: none) == "article")
  assert(articles.len() == 12, message: "All deep articles are rendered")
  let titles = query(heading).filter(it => it.at("label", default: none) == label("typsite-pdf-title"))
  assert(titles.map(it => it.level) == range(1, 13))
  assert(query(metadata).filter(it => it.value == "deepest").len() == 1)
}
"#,
    );
    for index in 0..12 {
        let next = if index == 11 {
            "#metadata(\"deepest\")".into()
        } else {
            format!("= #embed(\"./node-{}.typ\")", index + 1)
        };
        project.source(
            &format!("node-{index}.typ"),
            &format!("{LIB}\n#title[Node {index}]\n{next}"),
        );
    }
    let options = PdfOptions {
        entry: "node-0.typ".into(),
        ..PdfOptions::default()
    };
    assert!(
        compiler(&project, runner, options, &output)
            .compile()
            .unwrap()
            .written
    );
}

#[test]
#[ignore = "requires latest stable Typst; CI runs this explicitly"]
fn native_book_errors_retain_the_last_successful_pdf() {
    let (project, runner, output) = setup("");
    project.source(
        "index.typ",
        &format!("{LIB}\n#title[Root]\n= #embed(\"./child.typ\")"),
    );
    project.source("child.typ", &format!("{LIB}\n#title[Child]\nChild body"));
    let compiler = compiler(&project, runner, PdfOptions::default(), &output);
    compiler.compile().unwrap();
    let good = fs::read(&output).unwrap();
    for (root, child, diagnostic) in [
        (
            "#embed(\"../escape.typ\")",
            "Child body",
            "escapes the source root",
        ),
        ("#embed(\"missing.typ\")", "Child body", "not found"),
        (
            "#embed(\"child.typ\", heading-level: 0)",
            "Child body",
            "heading level must be positive",
        ),
        (
            "#embed(\"./child.typ\")",
            "#embed(\"/index.typ\")",
            "Recursive PDF embed",
        ),
        (
            "#embed(\"child.typ\", heading-level: 1)\n#embed(\"child.typ\", heading-level: 1)",
            "#set math.equation(numbering: \"(1)\")\n$ x = 1 $ <repeated>\nSee @repeated.",
            "Native Typst labels are document-global",
        ),
        (
            "#metacontent(\"loop\")[#get-metacontent(\"loop\")]\n#get-metacontent(\"loop\")",
            "Child body",
            "Recursive PDF metadata lookup",
        ),
        (
            "#context [\n= #embed(\"child.typ\")\n]",
            "Child body",
            "opaque context",
        ),
        (
            "#cite-title(\"child.typ\")",
            "Child body",
            "outside the PDF",
        ),
    ] {
        project.source("index.typ", &format!("{LIB}\n#title[Root]\n{root}"));
        project.source("child.typ", &format!("{LIB}\n#title[Child]\n{child}"));
        let error = format!("{:#}", compiler.compile().unwrap_err());
        assert!(error.contains(diagnostic), "Expected {diagnostic}: {error}");
        assert_eq!(
            fs::read(&output).unwrap(),
            good,
            "{diagnostic} must retain the last good PDF"
        );
    }
    // An opaque context can contain a plain embed with an explicit level.
    project.source(
        "index.typ",
        &format!("{LIB}\n#title[Root]\n#context [#embed(\"child.typ\", heading-level: 1)]"),
    );
    project.source(
        "child.typ",
        &format!("{LIB}\n#title[Child]\nRecovered child"),
    );
    assert!(compiler.compile().unwrap().written);
}

#[test]
#[ignore = "requires latest stable Typst; CI runs this explicitly"]
fn excluded_citations_use_encoded_website_urls_and_custom_book_styles() {
    let (project, runner, output) = setup(
        r#"
#context {
  let articles = query(metadata).filter(it => type(it.value) == dictionary and it.value.at("typsite-pdf", default: none) == "article")
  assert(articles.map(it => it.value.scope.source) == ("/index", "/child", "/lib/appendix.v1"), message: "Citations must not expand article bodies")
  assert(query(metadata).filter(it => it.value == "default-parent-used").len() == 1, message: "Default parent with a dotted filename")
  assert(query(link).any(it => it.dest == "https://example.org/notes/outside%20page.html"))
  assert(query(link).any(it => it.dest == "https://example.org/notes/outside%20page.html#section"))
  assert(calc.abs(page.width - 148mm) < 0.01pt, message: "Custom A5 template")
}
"#,
    );
    project.source("index.typ", &format!("{LIB}\n#title[Root]\n#get-metacontent(\"author\")\n#cite-title(\"outside page.typ\")\n#cite(\"outside page.typ\", anchor: \"section\")[External citation]\n#embed(\"child.typ\", heading-level: 1)"));
    project.source("child.typ", &format!("{LIB}\n#title[Child]\nNative body"));
    project.source(
        "outside page.typ",
        "#panic(\"Must not compile excluded source\")",
    );
    project.source(
        "lib/print.typ",
        "#let book-style(body) = { set page(paper: \"a5\"); set text(size: 9pt); body }",
    );
    project.source(
        "lib/appendix.v1.typ",
        &format!(
            "{LIB}\n#title[Standalone print appendix]\n#author[Print author#metadata(\"default-parent-used\")]\nNot implicitly compiled as an HTML article."
        ),
    );
    let config = fs::read_to_string(project.config.join("options.toml"))
        .unwrap()
        .replace(
            "base_url = \"\"",
            "base_url = \"https://example.org/notes\"",
        )
        .replace("parent = \"\"", "parent = \"content/lib/appendix.v1.typ\"");
    project.configure("options.toml", &config);
    let options = PdfOptions {
        appendices: vec!["lib/appendix.v1.typ".into()],
        template: Some("./lib/print.typ".into()),
        title: Some("A \"quoted\" \\ book\nUTF-8: λ".into()),
        pretty_url: false,
        ..PdfOptions::default()
    };
    assert!(
        compiler(&project, runner, options, &output)
            .compile()
            .unwrap()
            .written
    );
}

#[test]
#[ignore = "requires latest stable Typst; CI runs this explicitly"]
fn pdf_cli_does_not_touch_html_outputs_and_is_a_true_noop() {
    let (project, _, output) = setup("");
    project.source(
        "index.typ",
        &format!("{LIB}\n#title[CLI book]\n= #embed(\"child.typ\")"),
    );
    project.source("child.typ", &format!("{LIB}\n#title[Child]\nCLI child"));
    fs::create_dir_all(&project.output).unwrap();
    let ownership = br#"{"files":["index.html"]}"#;
    fs::write(project.output.join(".typsite-outputs.json"), ownership).unwrap();
    fs::write(project.output.join("index.html"), "Keep the site").unwrap();
    let fonts = project.root.join("extra fonts");
    fs::create_dir_all(&fonts).unwrap();
    let font = project.config.join("assets/fonts/Inria Sans.ttf");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&font, fonts.join("font.ttf")).unwrap();
    #[cfg(not(unix))]
    fs::copy(&font, fonts.join("font.ttf")).unwrap();
    let run = || {
        let result = Command::new(env!("CARGO_BIN_EXE_typsite"))
            .args(["pdf", "--input"])
            .arg(&project.input)
            .arg("--config")
            .arg(&project.config)
            .arg("--cache")
            .arg(&project.cache)
            .arg("--output")
            .arg(&output)
            .env("TYPST_PACKAGE_PATH", project.root.join("cli packages"))
            .env(
                "TYPST_PACKAGE_CACHE_PATH",
                project.root.join("cli package-cache"),
            )
            .env("TYPST_IGNORE_SYSTEM_FONTS", "true")
            .env("SOURCE_DATE_EPOCH", "315532800")
            .env("TYPST_FONT_PATHS", &fonts)
            .env_remove("TYPST_IGNORE_EMBEDDED_FONTS")
            .arg("--typst")
            .arg(std::env::var_os("TYPST").unwrap_or_else(|| "typst".into()))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout).unwrap()
    };
    assert!(run().contains("(compiled)"));
    let modified = fs::metadata(&output).unwrap().modified().unwrap();
    assert!(run().contains("(cached)"));
    assert_eq!(fs::metadata(&output).unwrap().modified().unwrap(), modified);
    assert_eq!(
        fs::read(project.output.join(".typsite-outputs.json")).unwrap(),
        ownership
    );
    assert_eq!(
        fs::read_to_string(project.output.join("index.html")).unwrap(),
        "Keep the site"
    );
}

#[test]
#[ignore = "requires latest stable Typst; CI runs this explicitly"]
fn named_footnotes_are_reference_only_and_restart_on_each_pdf_page() {
    let (project, runner, output) = setup(
        r#"
#context {
  let notes = query(footnote).filter(it => type(it.body) == content)
  assert(notes.len() == 4, message: "Definitions must not leave phantom notes: " + repr(notes.map(it => it.body)))
  assert(notes.map(it => counter(footnote).at(it.location())) == ((1,), (2,), (1,), (2,)), message: "Footnotes restart on each physical page")
  assert(notes.first().location().page() != notes.at(2).location().page())
  assert(query(metadata).filter(it => type(it.value) == dictionary and it.value.at("typsite-pdf", default: none) == "note-definition").len() == 1)
}
"#,
    );
    project.source(
        "index.typ",
        &format!("{LIB}\n{}", include_str!("fixtures/pdf-footnotes.typ")),
    );
    let compiler = compiler(&project, runner, PdfOptions::default(), &output);
    assert!(compiler.compile().unwrap().written);
    assert!(!compiler.compile().unwrap().compiled);
}

#[test]
#[ignore = "requires latest stable Typst; CI runs this explicitly"]
fn initialized_demo_compiles_into_a_native_book_with_appendices() {
    let (mut project, runner, output) = setup(
        r#"
#context {
  let articles = query(metadata).filter(it => type(it.value) == dictionary and it.value.at("typsite-pdf", default: none) == "article")
  assert(articles.map(it => it.value.scope.source) == ("/index", "/typst", "/article", "/migrate-to-016", "/migrate-to-017", "/migrate-to-018", "/example", "/typst"))
}
"#,
    );
    fs::remove_dir_all(&project.config).unwrap();
    fs::remove_dir_all(&project.input).unwrap();
    typsite::resource::default::copy_default_typsite(&project.root).unwrap();
    project.input = project.root.join("root");
    let options = PdfOptions {
        appendices: vec![
            "migrate-to-016.typ".into(),
            "migrate-to-017.typ".into(),
            "migrate-to-018.typ".into(),
            "example.typ".into(),
        ],
        template: Some("lib/pdf.typ".into()),
        ..PdfOptions::default()
    };
    let compiler = compiler(&project, runner, options, &output);
    assert!(compiler.compile().unwrap().written);
    assert!(!compiler.compile().unwrap().compiled);
}

#[test]
#[ignore = "requires latest stable Typst; CI runs this explicitly"]
fn standalone_native_helpers_do_not_require_the_html_feature() {
    let (project, runner, output) = setup("");
    project.source(
        "index.typ",
        &format!(
            r#"{LIB}
#title[Standalone PDF]
#metacontent("summary")[Invisible until requested#metadata("summary-used")]
#get-metacontent("summary")
Native math $x^2$ and #LaTeX.
#html.span[Native container fallback]
#context assert(query(metadata).filter(it => it.value == "summary-used").len() == 1)
"#
        ),
    );
    runner.prepare(None).unwrap();
    fs::create_dir_all(output.parent().unwrap()).unwrap();
    let result = Command::new(std::env::var_os("TYPST").unwrap_or_else(|| "typst".into()))
        .arg("compile")
        .arg(project.input.join("index.typ"))
        .arg(&output)
        .arg("--root")
        .arg(&project.input)
        .arg("--package-path")
        .arg(project.root.join("package namespace"))
        .arg("--font-path")
        .arg(project.config.join("assets/fonts"))
        .arg("--ignore-system-fonts")
        .env_remove("TYPST_FEATURES")
        .env_remove("TYPST_FONT_PATHS")
        .env("SOURCE_DATE_EPOCH", "315532800")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(fs::read(output).unwrap().starts_with(b"%PDF-"));
}
