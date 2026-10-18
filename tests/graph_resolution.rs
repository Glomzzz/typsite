mod common;

use common::{FakeTypst, Project, html, successful};
use std::fs;
use std::sync::Arc;

#[test]
fn explicit_self_parent_self_embed_and_metadata_cycles_are_errors() {
    for source in [
        "<metagraph key=\"parent\" slug=\"/a\"></metagraph>",
        "<embed slug=\"/a\"></embed>",
        "<metacontent set=\"title\"><rewrite id=\"cite\" slug=\"/a\">Self</rewrite></metacontent>",
    ] {
        let project = Project::new();
        project.source("a.typ", &html(source));
        let compiler = project.compiler(Arc::new(FakeTypst::default()));
        let report = compiler.compile().unwrap();
        assert!(!report.is_success(), "{source}");
        assert!(!project.output.join("a.html").exists());
    }
}

#[test]
fn cross_article_metadata_cycles_are_rejected_but_plain_self_citations_are_allowed() {
    let project = Project::new();
    for (page, target) in [("a", "b"), ("b", "a")] {
        project.source(&format!("{page}.typ"), &html(&format!("<metacontent set=\"title\"><rewrite id=\"cite\" slug=\"/{target}\">Target</rewrite></metacontent>")));
    }
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    assert!(!compiler.compile().unwrap().is_success());
    project.source(
        "a.typ",
        &html("<rewrite id=\"cite\" slug=\"/a\">Self</rewrite>"),
    );
    project.source("b.typ", &html("B"));
    successful(&compiler);
}

#[test]
fn deleting_the_last_article_does_not_require_removing_the_default_parent_setting() {
    let project = Project::new();
    let options = fs::read_to_string(project.config.join("options.toml")).unwrap();
    assert!(options.contains("parent = \"\""));
    project.configure(
        "options.toml",
        &options.replace("parent = \"\"", "parent = \"/index\""),
    );
    project.source("index.typ", &html("Home"));
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    fs::remove_file(project.input.join("index.typ")).unwrap();
    assert!(successful(&compiler).removed.contains(&"index.html".into()));
    assert!(!project.output.join("index.html").exists());
    assert!(!successful(&compiler).changed());
}
