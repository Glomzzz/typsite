mod common;

use common::{FakeTypst, Project, clean_equivalent, html, successful};
use std::fs;
use std::sync::Arc;
use typsite::compile::options::CompileOptions;

#[test]
fn racing_source_edits_cannot_leave_a_falsely_current_export() {
    let project = Project::new();
    project.source("a.typ", &html("First"));
    let runner = Arc::new(FakeTypst::default());
    *runner.edit_during_export.lock().unwrap() =
        Some((project.input.join("a.typ"), html("Edited during export")));
    let compiler = project.compiler(runner);
    assert!(!compiler.compile().unwrap().is_success());
    successful(&compiler);
    assert!(
        fs::read_to_string(project.output.join("a.html"))
            .unwrap()
            .contains("Edited during export")
    );
    clean_equivalent(&project, &compiler);
}

#[test]
fn racing_external_dependency_edits_are_verified_before_caching() {
    let project = Project::new();
    project.write(project.root.join("external.typ"), "First");
    project.source("a.typ", &html("{{include}}"));
    let runner = Arc::new(FakeTypst::default());
    runner.depends_on("a.typ", "../external.typ");
    *runner.edit_during_export.lock().unwrap() = Some((
        project.root.join("external.typ"),
        "Edited externally".into(),
    ));
    let compiler = project.compiler(runner.clone());
    successful(&compiler);
    assert_eq!(
        runner.calls().len(),
        2,
        "new external dependencies need a verification export"
    );
    assert!(
        fs::read_to_string(project.output.join("a.html"))
            .unwrap()
            .contains("Edited externally")
    );
    assert!(!successful(&compiler).changed());
    clean_equivalent(&project, &compiler);
}

#[test]
fn racing_library_edits_invalidate_the_whole_export_checkpoint() {
    let project = Project::new();
    project.source("lib/shared.typ", "First");
    project.source("a.typ", &html("{{include}}"));
    let runner = Arc::new(FakeTypst::default());
    runner.depends_on("a.typ", "lib/shared.typ");
    *runner.edit_during_export.lock().unwrap() = Some((
        project.input.join("lib/shared.typ"),
        "Edited library".into(),
    ));
    let compiler = project.compiler(runner);
    assert!(!compiler.compile().unwrap().is_success());
    successful(&compiler);
    assert!(
        fs::read_to_string(project.output.join("a.html"))
            .unwrap()
            .contains("Edited library")
    );
    clean_equivalent(&project, &compiler);
}

#[test]
fn owned_files_can_become_directories_and_back_again_without_a_clean() {
    let project = Project::new();
    project.source("slot", "asset file");
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    fs::remove_file(project.input.join("slot")).unwrap();
    project.source("slot/a.typ", &html("Nested page"));
    successful(&compiler);
    assert!(project.output.join("slot/a.html").is_file());
    fs::remove_dir_all(project.input.join("slot")).unwrap();
    project.source("slot", "asset again");
    successful(&compiler);
    assert_eq!(
        fs::read_to_string(project.output.join("slot")).unwrap(),
        "asset again"
    );
    clean_equivalent(&project, &compiler);
}

#[test]
fn directory_transitions_never_remove_unowned_children() {
    let project = Project::new();
    project.source("slot/a.typ", &html("Page"));
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    project.write(project.output.join("slot/manual.txt"), "User file");
    fs::remove_dir_all(project.input.join("slot")).unwrap();
    project.source("slot", "asset");
    assert!(!compiler.compile().unwrap().is_success());
    assert_eq!(
        fs::read_to_string(project.output.join("slot/manual.txt")).unwrap(),
        "User file"
    );
    fs::remove_file(project.output.join("slot/manual.txt")).unwrap();
    // The empty directory left by the partial publication is repaired too.
    successful(&compiler);
}

#[test]
fn internal_output_names_are_reserved_case_insensitively() {
    for path in [
        ".TYPSITE.LOCK",
        ".typsite.lock/nested",
        ".TYPSITE-OUTPUTS.JSON",
    ] {
        let project = Project::new();
        project.source(path, "must not replace internal state");
        let compiler = project.compiler(Arc::new(FakeTypst::default()));
        assert!(compiler.compile().is_err(), "{path}");
    }
}

#[test]
fn concurrent_compilers_do_not_share_configuration_or_render_state() {
    let first = Project::new();
    let second = Project::new();
    for (project, title) in [(&first, "First Project"), (&second, "Second Project")] {
        let options = fs::read_to_string(project.config.join("options.toml")).unwrap();
        project.configure(
            "options.toml",
            &options.replace("title = \"Untitled\"", &format!("title = \"{title}\"")),
        );
        project.configure(
            "schemas/main.html",
            "<head></head><body>{title}|{slug}|{slug-display}<content></content></body>",
        );
        project.source("nested/a.typ", &html("Shared slug"));
    }
    let a = first.compiler(Arc::new(FakeTypst::default()));
    let b = second.compiler_with_options(
        Arc::new(FakeTypst::default()),
        CompileOptions {
            pretty_url: false,
            short_slug: false,
            ..CompileOptions::default()
        },
    );
    std::thread::scope(|scope| {
        scope.spawn(|| successful(&a));
        scope.spawn(|| successful(&b));
    });
    assert!(
        fs::read_to_string(first.output.join("nested/a.html"))
            .unwrap()
            .contains("First Project|/nested/a|a")
    );
    assert!(
        fs::read_to_string(second.output.join("nested/a.html"))
            .unwrap()
            .contains("Second Project|/nested/a.html|/nested/a")
    );
}

#[test]
fn build_locks_reject_concurrent_access_instead_of_racing_manifests() {
    let project = Project::new();
    project.source("a.typ", &html("A"));
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    fs::create_dir_all(&project.output).unwrap();
    let lock = fs::File::create(project.output.join(".typsite.lock")).unwrap();
    lock.lock().unwrap();
    assert!(compiler.compile().is_err());
    lock.unlock().unwrap();
    successful(&compiler);
}

#[test]
fn ownership_paths_are_portable_and_legacy_windows_separators_are_readable() {
    let project = Project::new();
    project.source("nested/a.typ", &html("Nested"));
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    let path = project.output.join(".typsite-outputs.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert!(
        manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "nested/a.html")
    );
    for file in manifest["files"].as_array_mut().unwrap() {
        *file = serde_json::Value::String(file.as_str().unwrap().replace('/', "\\"));
    }
    fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(!successful(&compiler).changed());
    fs::remove_file(project.input.join("nested/a.typ")).unwrap();
    successful(&compiler);
    assert!(!project.output.join("nested/a.html").exists());
}

#[test]
fn corrupt_ownership_metadata_never_guesses_which_files_are_owned() {
    let project = Project::new();
    project.source("a.typ", &html("A"));
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    project.write(project.output.join("manual.txt"), "User file");
    fs::write(project.output.join(".typsite-outputs.json"), "invalid JSON").unwrap();
    fs::remove_file(project.input.join("a.typ")).unwrap();
    if let Ok(report) = compiler.compile() {
        assert!(!report.is_success());
    }
    assert!(project.output.join("a.html").exists());
    assert!(project.output.join("manual.txt").exists());
    fs::remove_file(project.output.join(".typsite-outputs.json")).unwrap();
    successful(&compiler);
    assert!(
        project.output.join("a.html").exists(),
        "removing ownership metadata explicitly gives up ownership of old files"
    );
}
