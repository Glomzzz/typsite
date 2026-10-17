mod common;

use common::{FakeTypst, Project, html};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use typsite::compile::compiler::Compiler;
use typsite::compile::project::ProjectPaths;
use typsite::compile::report::{BuildReport, Stage};

fn successful(compiler: &Compiler) -> BuildReport {
    let report = compiler
        .compile()
        .expect("build infrastructure should succeed");
    assert!(report.is_success(), "{:#?}", report.diagnostics);
    report
}

fn clean_equivalent(project: &Project, compiler: &Compiler) {
    let incremental = project.snapshot();
    fs::remove_dir_all(&project.cache).unwrap();
    fs::remove_dir_all(&project.output).unwrap();
    successful(compiler);
    assert_eq!(
        incremental,
        project.snapshot(),
        "incremental output must equal a clean build"
    );
}

#[test]
fn cold_build_and_true_noop_build() {
    let project = Project::new();
    project.source("a.typ", &html("A"));
    project.source("b.typ", &html("B"));
    for asset in ["notes.txt", "picture.jpg", "site.map", "empty"] {
        project.source(asset, "");
    }
    let runner = Arc::new(FakeTypst::default());
    let compiler = project.compiler(runner.clone());
    let first = successful(&compiler);
    assert_eq!(first.exported.len(), 2);
    assert_eq!(first.rendered, 2);
    assert!(first.changed());
    let before = project.snapshot();
    let modified = fs::metadata(project.output.join("a.html"))
        .unwrap()
        .modified()
        .unwrap();
    let second = successful(&compiler);
    assert!(!second.changed());
    assert_eq!(second.rendered, 0);
    assert!(second.exported.is_empty());
    assert_eq!(runner.calls().len(), 2);
    assert_eq!(before, project.snapshot());
    assert_eq!(
        modified,
        fs::metadata(project.output.join("a.html"))
            .unwrap()
            .modified()
            .unwrap()
    );
    for asset in ["notes.txt", "picture.jpg", "site.map", "empty"] {
        assert!(project.output.join(asset).is_file());
    }
}

#[test]
fn removing_citations_removes_backlinks_and_matches_clean_builds() {
    let project = Project::new();
    project.source(
        "a.typ",
        &html("A <rewrite id=\"cite\" slug=\"/b\">B</rewrite>"),
    );
    project.source("b.typ", &html("B"));
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    assert!(
        fs::read_to_string(project.output.join("b.html"))
            .unwrap()
            .contains("Backlink /a")
    );
    project.source("a.typ", &html("A without a citation"));
    successful(&compiler);
    assert!(
        !fs::read_to_string(project.output.join("b.html"))
            .unwrap()
            .contains("Backlink /a")
    );
    clean_equivalent(&project, &compiler);
}

#[test]
fn mutual_citations_are_valid_and_output_is_deterministic() {
    let project = Project::new();
    for (page, target) in [("a", "b"), ("b", "a"), ("c", "a")] {
        project.source(
            &format!("{page}.typ"),
            &html(&format!(
                "{page} <rewrite id=\"cite\" slug=\"/{target}\">{target}</rewrite>"
            )),
        );
    }
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    for _ in 0..3 {
        clean_equivalent(&project, &compiler);
    }
}

#[test]
fn shared_embeds_recompose_without_stale_titles_or_footer_relations() {
    let project = Project::new();
    project.source(
        "a.typ",
        &html("<embed slug=\"/b\"></embed><embed slug=\"/c\"></embed>"),
    );
    project.source(
        "b.typ",
        &html("<metacontent set=\"title\">B title</metacontent><embed slug=\"/d\"></embed>"),
    );
    project.source("c.typ", &html("<embed slug=\"/d\"></embed>"));
    project.source("d.typ", &html("D content"));
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    project.source(
        "b.typ",
        &html("<metacontent set=\"title\">Changed B</metacontent><embed slug=\"/d\"></embed>"),
    );
    successful(&compiler);
    let page = fs::read_to_string(project.output.join("a.html")).unwrap();
    assert!(page.contains("Changed B"));
    assert!(page.contains("D content"));
    assert!(!page.contains("B title"));
    clean_equivalent(&project, &compiler);
    project.source("a.typ", &html("Removed all embeds"));
    successful(&compiler);
    clean_equivalent(&project, &compiler);
}

#[test]
fn recursive_embed_and_parent_graphs_report_errors_instead_of_hanging() {
    for kind in ["embed", "parent"] {
        let project = Project::new();
        for (page, other) in [("a", "b"), ("b", "a")] {
            let body = if kind == "embed" {
                format!("<embed slug=\"/{other}\"></embed>")
            } else {
                format!("<metagraph key=\"parent\" slug=\"/{other}\"></metagraph>")
            };
            project.source(&format!("{page}.typ"), &html(&body));
        }
        let compiler = project.compiler(Arc::new(FakeTypst::default()));
        let report = compiler.compile().unwrap();
        assert!(!report.is_success());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.stage == Stage::Resolve && d.message.contains("Recursive")),
            "{report:?}"
        );
        assert!(!project.output.join("a.html").exists());
    }
}

#[test]
fn inherited_metadata_updates_from_the_parent() {
    let project = Project::new();
    project.configure(
        "schemas/main.html",
        "<head><title>{title@parent}</title></head><body>{title@parent}<content></content></body>",
    );
    project.source(
        "parent.typ",
        &html("<metacontent set=\"title\">Original parent</metacontent>Parent"),
    );
    project.source(
        "child.typ",
        &html("<metagraph key=\"parent\" slug=\"/parent\"></metagraph>Child"),
    );
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    project.source(
        "parent.typ",
        &html("<metacontent set=\"title\">New parent</metacontent>Parent"),
    );
    successful(&compiler);
    assert!(
        fs::read_to_string(project.output.join("child.html"))
            .unwrap()
            .contains("New parent")
    );
    clean_equivalent(&project, &compiler);
}

#[test]
fn output_write_failures_are_reported_and_retried_without_cleaning() {
    let project = Project::new();
    project.source("a.typ", &html("A"));
    fs::create_dir_all(project.output.join("a.html")).unwrap();
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    let failed = compiler.compile().unwrap();
    assert!(!failed.is_success());
    assert!(failed.diagnostics.iter().any(|d| d.stage == Stage::Publish));
    fs::remove_dir(project.output.join("a.html")).unwrap();
    let repaired = successful(&compiler);
    assert!(
        repaired.exported.is_empty(),
        "successful exports should survive publication failure"
    );
    assert!(project.output.join("a.html").is_file());
}

#[test]
fn blocked_managed_roots_fail_truthfully_and_recover() {
    for root in ["cache", "publish"] {
        let project = Project::new();
        project.source("a.typ", &html("A"));
        let compiler = project.compiler(Arc::new(FakeTypst::default()));
        fs::write(project.root.join(root), "blocked").unwrap();
        assert!(compiler.compile().is_err());
        fs::remove_file(project.root.join(root)).unwrap();
        successful(&compiler);
    }
}

#[test]
fn failed_exports_do_not_replace_good_artifacts_or_publish_stale_html() {
    let project = Project::new();
    project.source("a.typ", &html("Original"));
    project.source("b.typ", &html("B"));
    let runner = Arc::new(FakeTypst::default());
    let compiler = project.compiler(runner.clone());
    successful(&compiler);
    let good = project.snapshot();
    let artifact = fs::read(project.cache.join("exports/a.html")).unwrap();
    project.source("a.typ", &html("Edited"));
    runner.fail_next.store(true, Ordering::SeqCst);
    let failed = compiler.compile().unwrap();
    assert!(!failed.is_success());
    assert!(failed.diagnostics.iter().any(|d| d.stage == Stage::Export));
    assert_eq!(
        artifact,
        fs::read(project.cache.join("exports/a.html")).unwrap()
    );
    assert_eq!(good, project.snapshot());
    let repaired = successful(&compiler);
    assert_eq!(repaired.exported, vec![Path::new("a.typ")]);
    assert!(
        fs::read_to_string(project.output.join("a.html"))
            .unwrap()
            .contains("Edited")
    );
    clean_equivalent(&project, &compiler);
}

#[test]
fn corrupt_missing_and_incompatible_caches_are_cache_misses() {
    let project = Project::new();
    project.source("a.typ", &html("A"));
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    let original = project.snapshot();
    for metadata in [
        "not json",
        "{\"version\":999,\"export_key\":\"\",\"exports\":{},\"published\":null}",
    ] {
        fs::write(project.cache.join("manifest.json"), metadata).unwrap();
        assert_eq!(successful(&compiler).exported.len(), 1);
        assert_eq!(original, project.snapshot());
    }
    fs::write(project.cache.join("exports/a.html"), "corrupt").unwrap();
    assert_eq!(successful(&compiler).exported.len(), 1);
    fs::remove_file(project.cache.join("exports/a.html")).unwrap();
    assert_eq!(successful(&compiler).exported.len(), 1);
    fs::remove_file(project.output.join("a.html")).unwrap();
    let repaired = successful(&compiler);
    assert!(repaired.exported.is_empty());
    assert!(repaired.changed());
    assert_eq!(original, project.snapshot());
}

#[test]
fn real_dependency_metadata_drives_selective_typst_export() {
    let project = Project::new();
    project.source("lib/shared.typ", "First library value");
    project.source("a.typ", &html("{{include}}"));
    project.source("b.typ", &html("B"));
    let runner = Arc::new(FakeTypst::default());
    runner.depends_on("a.typ", "lib/shared.typ");
    let compiler = project.compiler(runner.clone());
    successful(&compiler);
    assert!(!project.output.join("lib/shared.html").exists());
    project.source("lib/shared.typ", "Changed library value");
    let changed = successful(&compiler);
    assert_eq!(changed.exported, vec![Path::new("a.typ")]);
    assert!(
        fs::read_to_string(project.output.join("a.html"))
            .unwrap()
            .contains("Changed library value")
    );
    clean_equivalent(&project, &compiler);
    fs::remove_file(project.input.join("lib/shared.typ")).unwrap();
    let failed = compiler.compile().unwrap();
    assert!(!failed.is_success());
    project.source("lib/shared.typ", "Restored");
    successful(&compiler);
}

#[test]
fn imports_of_other_articles_are_tracked_even_without_library_declarations() {
    let project = Project::new();
    project.source("a.typ", &html("A"));
    project.source("b.typ", &html("B"));
    let runner = Arc::new(FakeTypst::default());
    runner.depends_on("a.typ", "b.typ");
    let compiler = project.compiler(runner);
    successful(&compiler);
    project.source("b.typ", &html("Changed B"));
    assert_eq!(successful(&compiler).exported.len(), 2);
}

#[test]
fn template_changes_do_not_reexport_typst_and_footer_templates_stay_distinct() {
    let project = Project::new();
    project.source(
        "a.typ",
        &html("<rewrite id=\"cite\" slug=\"/b\">B</rewrite>"),
    );
    project.source("b.typ", &html("B"));
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    project.configure(
        "components/footer/references.html",
        "<head></head><body><div class=\"changed-references\">{references}</div></body>",
    );
    let report = successful(&compiler);
    assert!(report.exported.is_empty());
    let a = fs::read_to_string(project.output.join("a.html")).unwrap();
    let b = fs::read_to_string(project.output.join("b.html")).unwrap();
    assert!(a.contains("changed-references"));
    assert!(!b.contains("changed-references"));
    assert!(b.contains("backlinks"));
    clean_equivalent(&project, &compiler);
}

#[test]
fn malformed_articles_and_missing_templates_never_report_success() {
    let project = Project::new();
    project.source("a.typ", "<head></head><body>Missing schema</body>");
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    assert!(!compiler.compile().unwrap().is_success());
    project.source(
        "a.typ",
        &html("<rewrite id=\"cite\" slug=\"/missing\">Missing</rewrite>"),
    );
    assert!(!compiler.compile().unwrap().is_success());
    project.source("a.typ", &html("Good"));
    successful(&compiler);
    let good = project.snapshot();
    fs::remove_file(project.config.join("components/section.html")).unwrap();
    assert!(compiler.compile().is_err());
    assert_eq!(good, project.snapshot());
}

#[test]
fn deletions_and_renames_clean_owned_outputs_even_after_cache_loss() {
    let project = Project::new();
    project.source("old.typ", &html("Old"));
    project.source("static.txt", "asset");
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    project.write(
        project.output.join("manual.txt"),
        "not generated by Typsite",
    );
    fs::rename(project.input.join("old.typ"), project.input.join("new.typ")).unwrap();
    fs::remove_file(project.input.join("static.txt")).unwrap();
    fs::remove_dir_all(&project.cache).unwrap();
    let report = successful(&compiler);
    assert!(report.removed.contains(&"old.html".into()));
    assert!(report.removed.contains(&"static.txt".into()));
    assert!(project.output.join("new.html").exists());
    assert!(project.output.join("manual.txt").exists());
    fs::remove_file(project.input.join("new.typ")).unwrap();
    successful(&compiler);
    assert!(!project.output.join("new.html").exists());
}

#[test]
fn asset_only_edits_and_empty_sites_produce_accurate_change_reports() {
    let project = Project::new();
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    project.source("asset.txt", "first");
    let changed = successful(&compiler);
    assert!(changed.changed());
    assert!(changed.exported.is_empty());
    assert_eq!(changed.rendered, 0);
    fs::remove_file(project.input.join("asset.txt")).unwrap();
    assert!(successful(&compiler).changed());
    assert!(!successful(&compiler).changed());
}

#[test]
fn generated_feeds_are_updated_removed_and_confined_to_output() {
    let project = Project::new();
    project.source("a.typ", &html("A"));
    let options = fs::read_to_string(project.config.join("options.toml")).unwrap();
    project.configure(
        "options.toml",
        &options.replace("base_url = \"\"", "base_url = \"https://example.org\""),
    );
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    assert!(
        fs::read_to_string(project.output.join("sitemap.xml"))
            .unwrap()
            .contains("https://example.org/a")
    );
    assert!(project.output.join("rss.xml").is_file());
    project.configure("options.toml", &options);
    successful(&compiler);
    assert!(!project.output.join("rss.xml").exists());
    assert!(!project.output.join("sitemap.xml").exists());
    let bad = options
        .replace("base_url = \"\"", "base_url = \"https://example.org\"")
        .replace("rss_path = \"/rss.xml\"", "rss_path = \"../escape.xml\"");
    project.configure("options.toml", &bad);
    assert!(compiler.compile().is_err());
    assert!(!project.root.join("escape.xml").exists());
}

#[test]
fn output_collisions_and_overlapping_roots_are_rejected() {
    let project = Project::new();
    project.source("a.typ", &html("A"));
    project.source("a.html", "would overwrite the generated page");
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    assert!(
        compiler
            .compile()
            .unwrap_err()
            .to_string()
            .contains("collision")
    );
    assert!(
        ProjectPaths::new(
            project.input.clone(),
            project.config.clone(),
            project.input.join("cache"),
            project.output.clone(),
            None
        )
        .is_err()
    );
}

#[test]
fn changing_typst_identity_invalidates_exports() {
    let project = Project::new();
    project.source("a.typ", &html("A"));
    let runner = Arc::new(FakeTypst::default());
    let compiler = project.compiler(runner.clone());
    successful(&compiler);
    runner.revision.fetch_add(1, Ordering::SeqCst);
    assert_eq!(successful(&compiler).exported.len(), 1);
}

#[test]
fn source_directories_named_like_the_input_root_keep_their_full_slug() {
    let project = Project::new();
    project.source("content/a.typ", &html("Nested"));
    project.source("a.typ", &html("Top level"));
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    assert!(project.output.join("content/a.html").is_file());
    assert!(project.output.join("a.html").is_file());
}

#[cfg(unix)]
#[test]
fn output_symlinks_cannot_write_or_delete_outside_the_managed_root() {
    let project = Project::new();
    project.source("nested/a.typ", &html("A"));
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    fs::create_dir_all(&project.output).unwrap();
    let outside = project.root.join("outside");
    fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, project.output.join("nested")).unwrap();
    match compiler.compile() {
        Ok(report) => assert!(!report.is_success()),
        Err(_) => {}
    }
    assert!(!outside.join("a.html").exists());
}
