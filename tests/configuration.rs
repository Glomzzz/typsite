mod common;

use common::{FakeTypst, Project, html, successful};
use std::fs;
use std::sync::Arc;

#[test]
fn malformed_resources_are_errors_even_if_no_article_uses_them() {
    for (path, content) in [
        ("schemas/unused.html", "No head or body"),
        (
            "rewrite/unused.html",
            "<rewrite pass=\"unknown\"></rewrite><head></head><body>{body}</body>",
        ),
        ("themes/unused.tmTheme", "Not a theme"),
        (
            "syntaxes/unused.sublime-syntax",
            "name: Invalid\ncontexts: [not a map]",
        ),
        ("syntaxes/unused.tmPreferences", "Not a property list"),
    ] {
        let project = Project::new();
        project.source("a.typ", &html("A"));
        let compiler = project.compiler(Arc::new(FakeTypst::default()));
        successful(&compiler);
        let previous = project.snapshot();
        project.configure(path, content);
        let error = compiler.compile().unwrap_err();
        assert!(
            format!("{error:#}").contains(path.rsplit('/').next().unwrap()),
            "{error:#}"
        );
        assert_eq!(previous, project.snapshot());
        fs::remove_file(project.config.join(path)).unwrap();
        successful(&compiler);
    }
}

#[test]
fn malformed_shell_variables_do_not_disappear_into_log_messages() {
    let project = Project::new();
    project.configure(
        "syntaxes/invalid.tmPreferences",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>scope</key><string>source.rust</string>
<key>settings</key><dict><key>shellVariables</key><string>not a list</string></dict>
</dict></plist>"#,
    );
    project.source("a.typ", &html("A"));
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    let error = compiler.compile().unwrap_err();
    assert!(
        format!("{error:#}").contains("shell variables"),
        "{error:#}"
    );
}

#[test]
fn duplicate_resource_ids_are_rejected_deterministically() {
    let project = Project::new();
    project.source("a.typ", &html("A"));
    project.configure(
        "schemas/nested/main.html",
        "<head></head><body>Duplicate main schema</body>",
    );
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    assert!(format!("{:#}", compiler.compile().unwrap_err()).contains("Duplicate schema main"));
}

#[test]
fn project_paths_with_spaces_and_glob_metacharacters_are_literal_paths() {
    let project = Project::named("typsite [literal] paths with spaces");
    project.source("nested/a.typ", &html("A"));
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    assert!(project.output.join("nested/a.html").is_file());
}
