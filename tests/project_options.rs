mod common;

use common::Project;
use std::collections::HashMap;
use typsite::compile::options::{CompileOptions, RssContent};
use typsite::ir::metadata::content::MetaContents;

#[test]
fn independent_projects_keep_options_and_metadata_defaults() {
    let first = Project::new();
    let second = Project::new();
    let options = std::fs::read_to_string(second.config.join("options.toml")).unwrap();
    second.configure(
        "options.toml",
        &options.replace("Untitled", "Other Project"),
    );
    let first = first.load(CompileOptions::default());
    let second = second.load(CompileOptions {
        pretty_url: false,
        short_slug: false,
        watch: true,
    });
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                let metadata =
                    MetaContents::new("/nested/page".into(), HashMap::new(), true, &first.options);
                assert_eq!(
                    metadata.inline_with("{title}|{slug}|{slug-display}", &[]),
                    "Untitled|/nested/page|page"
                );
            });
            scope.spawn(|| {
                let metadata =
                    MetaContents::new("/nested/page".into(), HashMap::new(), true, &second.options);
                assert_eq!(
                    metadata.inline_with("{title}|{slug}|{slug-display}", &[]),
                    "Other Project|/nested/page.html|/nested/page"
                );
            });
        }
    });
}

#[test]
fn shared_layout_is_optional_for_existing_projects() {
    let project = Project::new();
    let options = std::fs::read_to_string(project.config.join("options.toml")).unwrap();
    project.configure(
        "options.toml",
        &options
            .replace("[layout]", "# [layout]")
            .replace("module = \"\"", "# module = \"\""),
    );
    assert!(
        project
            .load(CompileOptions::default())
            .options
            .project
            .layout
            .module
            .is_empty()
    );
    project.configure(
        "options.toml",
        &options.replace("module = \"\"", "module = \"lib/custom.typ\""),
    );
    assert_eq!(
        project
            .load(CompileOptions::default())
            .options
            .project
            .layout
            .module,
        "lib/custom.typ"
    );
}

#[test]
fn missing_site_section_uses_the_same_defaults_as_an_empty_section() {
    let project = Project::new();
    let options = std::fs::read_to_string(project.config.join("options.toml")).unwrap();
    project.configure("options.toml", options.split("[site]").next().unwrap());
    let absent = project.load(CompileOptions::default()).options;
    project.configure(
        "options.toml",
        &format!("{}\n[site]\n", options.split("[site]").next().unwrap()),
    );
    let empty = project.load(CompileOptions::default()).options;
    assert_eq!(absent.project.site, empty.project.site);
    assert_eq!(absent.project.site.rss_limit, 20);
    assert_eq!(absent.project.site.rss_path, "/rss.xml");
    assert_eq!(absent.project.site.rss_content, RssContent::Summary);
    assert!(!absent.project.default_metadata.options.rss);
}
