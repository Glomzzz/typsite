mod common;

use common::Project;
use std::path::Path;
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
