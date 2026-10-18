mod common;

use common::{FakeTypst, Project, html, successful};
use std::fs;
use std::sync::Arc;
use typsite::compile::compiler::Compiler;
use typsite::compile::options::CompileOptions;
use typsite::compile::project::ProjectPaths;
use typsite::resource::package::{install_included_into, install_packages_into};

#[test]
fn package_installation_can_be_scoped_without_changing_home_or_environment() {
    let project = Project::new();
    let sources = project.root.join("package sources");
    let namespace = project.root.join("isolated namespace");
    project.write(
        sources.join("custom/typst.toml"),
        "[package]\nname = \"test-package\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"",
    );
    project.write(sources.join("custom/lib.typ"), "first");
    install_included_into(&namespace).unwrap();
    install_packages_into(&sources, &namespace).unwrap();
    assert!(namespace.join("local/typsite/0.1.0/typst.toml").exists());
    let installed = namespace.join("local/test-package/0.1.0/lib.typ");
    assert_eq!(fs::read_to_string(&installed).unwrap(), "first");
    project.write(sources.join("custom/lib.typ"), "second");
    install_packages_into(&sources, &namespace).unwrap();
    assert_eq!(fs::read_to_string(installed).unwrap(), "second");
}

#[test]
fn runner_preparation_returns_only_installed_roots_not_unrelated_local_packages() {
    use typsite::compile::typst::{CommandTypst, TypstRunner};
    let project = Project::new();
    let namespace = project.root.join("namespace");
    fs::create_dir_all(namespace.join("local/unrelated/1.0.0")).unwrap();
    fs::write(
        namespace.join("local/unrelated/1.0.0/manual"),
        "do not inspect or replace",
    )
    .unwrap();
    let runner = CommandTypst::new("typst").with_package_path(namespace.clone());
    let prepared = runner.prepare(None).unwrap();
    assert_eq!(prepared, vec![namespace.join("local/typsite/0.1.0")]);
    assert!(namespace.join("local/unrelated/1.0.0/manual").exists());
}

#[test]
fn invalid_package_identities_cannot_escape_the_installation_namespace() {
    for (name, version) in [
        ("../escape", "0.1.0"),
        ("valid", "../escape"),
        ("valid\\escape", "0.1.0"),
    ] {
        let project = Project::new();
        let sources = project.root.join("packages");
        let namespace = project.root.join("namespace");
        project.write(
            sources.join("bad/typst.toml"),
            &format!("[package]\nname = '{}'\nversion = '{}'", name, version),
        );
        assert!(install_packages_into(&sources, &namespace).is_err());
        assert!(
            !namespace.exists(),
            "validate all package metadata before installation"
        );
        assert!(!project.root.join("escape").exists());
    }
}

#[test]
fn duplicate_package_identities_fail_before_installation() {
    let project = Project::new();
    let sources = project.root.join("packages");
    let namespace = project.root.join("namespace");
    for path in ["first", "second"] {
        project.write(
            sources.join(path).join("typst.toml"),
            "[package]\nname = 'same'\nversion = '0.1.0'",
        );
    }
    assert!(
        install_packages_into(&sources, &namespace)
            .unwrap_err()
            .to_string()
            .contains("Duplicate")
    );
    assert!(!namespace.exists());
}

#[test]
fn package_source_changes_invalidate_exports_without_global_installation_in_tests() {
    let project = Project::new();
    let packages = project.root.join("packages");
    project.write(packages.join("custom/data.typ"), "First package value");
    project.source("a.typ", &html("{{include}}"));
    let runner = Arc::new(FakeTypst::default());
    runner.depends_on("a.typ", "../packages/custom/data.typ");
    let compiler = Compiler::with_runner(
        ProjectPaths::new(
            project.input.clone(),
            project.config.clone(),
            project.cache.clone(),
            project.output.clone(),
            Some(packages.clone()),
        )
        .unwrap(),
        CompileOptions::default(),
        runner,
    )
    .unwrap();
    successful(&compiler);
    assert!(!successful(&compiler).changed());
    project.write(packages.join("custom/data.typ"), "Changed package value");
    assert_eq!(successful(&compiler).exported.len(), 1);
    assert!(
        fs::read_to_string(project.output.join("a.html"))
            .unwrap()
            .contains("Changed package value")
    );
}
