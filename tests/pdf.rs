mod common;

use anyhow::Result;
use common::Project;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use typsite::compile::pdf::{PdfCompiler, PdfOptions};
use typsite::compile::typst::{ExportRequest, PdfRequest, TypstRunner};

#[derive(Default)]
struct BookRunner {
    calls: AtomicUsize,
    fail: AtomicBool,
    dependencies: Mutex<Vec<PathBuf>>,
    edit: Mutex<Option<(PathBuf, String)>>,
    identity_edit: Mutex<Option<(PathBuf, String)>>,
}

impl TypstRunner for BookRunner {
    fn identity(&self) -> Result<String> {
        if let Some((path, contents)) = self.identity_edit.lock().unwrap().take() {
            fs::write(path, contents)?;
        }
        Ok("fake PDF runner 1".into())
    }
    fn export(&self, _: &ExportRequest<'_>) -> Result<Vec<PathBuf>> {
        anyhow::bail!("Books must not export HTML")
    }
    fn pdf(&self, request: &PdfRequest<'_>) -> Result<Vec<PathBuf>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(request.root.is_absolute());
        assert!(request.driver.contains("pdf-book("));
        if self.fail.swap(false, Ordering::SeqCst) {
            fs::write(request.output, "partial failed PDF")?;
            anyhow::bail!("deliberate PDF failure");
        }
        let mut dependencies = vec![request.root.join("index.typ")];
        dependencies.extend(self.dependencies.lock().unwrap().iter().cloned());
        let mut bytes = format!("%PDF-1.7\n{}", request.driver).into_bytes();
        for path in &dependencies {
            bytes.extend(fs::read(path)?);
        }
        fs::write(request.output, bytes)?;
        if let Some((path, contents)) = self.edit.lock().unwrap().take() {
            fs::write(path, contents)?;
        }
        Ok(dependencies)
    }
}

fn compiler(
    project: &Project,
    runner: Arc<BookRunner>,
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

fn fixture() -> (Project, Arc<BookRunner>, PathBuf) {
    let project = Project::named("typsite PDF [fixture]");
    project.source("index.typ", "Initial book source");
    let output = project.root.join("books/site.pdf");
    (project, Arc::new(BookRunner::default()), output)
}

fn artifact(project: &Project, extension: &str) -> PathBuf {
    fs::read_dir(project.cache.join("pdf"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == extension))
        .unwrap()
}

#[test]
fn book_noops_and_missing_output_is_repaired_without_compiling() {
    let (project, runner, output) = fixture();
    let compiler = compiler(&project, runner.clone(), PdfOptions::default(), &output);
    let first = compiler.compile().unwrap();
    assert!(first.compiled && first.written);
    let bytes = fs::read(&output).unwrap();
    let modified = fs::metadata(&output).unwrap().modified().unwrap();
    let manifest = artifact(&project, "json");
    let cache_modified = fs::metadata(&manifest).unwrap().modified().unwrap();
    let warm = compiler.compile().unwrap();
    assert!(!warm.compiled && !warm.written);
    assert_eq!(fs::metadata(&output).unwrap().modified().unwrap(), modified);
    assert_eq!(
        fs::metadata(&manifest).unwrap().modified().unwrap(),
        cache_modified
    );
    fs::remove_file(&output).unwrap();
    let repaired = compiler.compile().unwrap();
    assert!(!repaired.compiled && repaired.written);
    assert_eq!(fs::read(&output).unwrap(), bytes);
    assert_eq!(runner.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn catalog_edits_racing_a_warm_build_are_not_silently_reported_as_current() {
    let (project, runner, output) = fixture();
    let compiler = compiler(&project, runner.clone(), PdfOptions::default(), &output);
    compiler.compile().unwrap();
    let good = fs::read(&output).unwrap();
    *runner.identity_edit.lock().unwrap() = Some((
        project.input.join("new-article.typ"),
        "New catalog entry".into(),
    ));
    assert!(
        compiler
            .compile()
            .unwrap_err()
            .to_string()
            .contains("Inputs changed")
    );
    assert_eq!(fs::read(&output).unwrap(), good);
    assert_eq!(runner.calls.load(Ordering::SeqCst), 1);
    assert!(compiler.compile().unwrap().written);
}

#[test]
fn failed_and_racing_compilations_retain_last_good_book() {
    let (project, runner, output) = fixture();
    let compiler = compiler(&project, runner.clone(), PdfOptions::default(), &output);
    compiler.compile().unwrap();
    let good = fs::read(&output).unwrap();
    project.source("index.typ", "Changed book source");
    runner.fail.store(true, Ordering::SeqCst);
    assert!(
        compiler
            .compile()
            .unwrap_err()
            .to_string()
            .contains("deliberate PDF failure")
    );
    assert_eq!(fs::read(&output).unwrap(), good);
    *runner.edit.lock().unwrap() = Some((project.input.join("index.typ"), "Racing edit".into()));
    assert!(
        compiler
            .compile()
            .unwrap_err()
            .to_string()
            .contains("Inputs changed")
    );
    assert_eq!(fs::read(&output).unwrap(), good);
    assert!(compiler.compile().unwrap().written);
    let incremental = fs::read(&output).unwrap();
    fs::remove_dir_all(&project.cache).unwrap();
    compiler.compile().unwrap();
    assert_eq!(fs::read(&output).unwrap(), incremental);
}

#[test]
fn actual_external_dependencies_are_verified_and_invalidate_the_book() {
    let (project, runner, output) = fixture();
    let external = project.root.join("external-library.typ");
    fs::write(&external, "First dependency").unwrap();
    runner.dependencies.lock().unwrap().push(external.clone());
    let compiler = compiler(&project, runner.clone(), PdfOptions::default(), &output);
    compiler.compile().unwrap();
    assert_eq!(
        runner.calls.load(Ordering::SeqCst),
        2,
        "Unknown dependencies need a verification compile"
    );
    assert!(!compiler.compile().unwrap().compiled);
    fs::write(&external, "Changed dependency").unwrap();
    assert!(compiler.compile().unwrap().compiled);
    assert_eq!(runner.calls.load(Ordering::SeqCst), 3);
}

#[test]
fn corrupt_book_caches_recover_without_a_clean() {
    let (project, runner, output) = fixture();
    let compiler = compiler(&project, runner.clone(), PdfOptions::default(), &output);
    compiler.compile().unwrap();
    let good = fs::read(&output).unwrap();
    fs::write(artifact(&project, "pdf"), "corrupt").unwrap();
    let repaired = compiler.compile().unwrap();
    assert!(repaired.compiled && !repaired.written);
    fs::write(artifact(&project, "json"), "{broken").unwrap();
    assert!(compiler.compile().unwrap().compiled);
    assert_eq!(fs::read(&output).unwrap(), good);
    assert_eq!(runner.calls.load(Ordering::SeqCst), 3);
}

#[test]
fn pdf_does_not_replace_html_ownership_or_remove_html_files() {
    let (project, runner, _) = fixture();
    fs::create_dir_all(&project.output).unwrap();
    let ownership = br#"{"files":["index.html"]}"#;
    fs::write(project.output.join(".typsite-outputs.json"), ownership).unwrap();
    fs::write(project.output.join("index.html"), "Last good site").unwrap();
    fs::write(project.output.join("unrelated.txt"), "Keep this").unwrap();
    let output = project.output.join("site.pdf");
    compiler(&project, runner, PdfOptions::default(), &output)
        .compile()
        .unwrap();
    assert_eq!(
        fs::read(project.output.join(".typsite-outputs.json")).unwrap(),
        ownership
    );
    assert_eq!(
        fs::read_to_string(project.output.join("index.html")).unwrap(),
        "Last good site"
    );
    assert!(project.output.join("unrelated.txt").is_file());
}

#[test]
fn book_rejects_protected_and_html_owned_output_files() {
    let (project, runner, output) = fixture();
    for output in [
        project.input.join("article.pdf"),
        project.config.join("article.pdf"),
        project.cache.join("article.pdf"),
    ] {
        assert!(
            PdfCompiler::with_runner(
                project.input.clone(),
                project.config.clone(),
                project.cache.clone(),
                output,
                None,
                PdfOptions::default(),
                runner.clone()
            )
            .is_err()
        );
    }
    fs::create_dir_all(output.parent().unwrap()).unwrap();
    fs::write(
        output.parent().unwrap().join(".typsite-outputs.json"),
        r#"{"files":["site.pdf"]}"#,
    )
    .unwrap();
    assert!(
        PdfCompiler::with_runner(
            project.input.clone(),
            project.config.clone(),
            project.cache.clone(),
            output,
            None,
            PdfOptions::default(),
            runner
        )
        .err()
        .unwrap()
        .to_string()
        .contains("owned by an HTML")
    );
}

#[test]
fn missing_and_escaping_roots_fail_but_explicit_standalone_pages_are_allowed() {
    let (project, runner, output) = fixture();
    for options in [
        PdfOptions {
            entry: "missing.typ".into(),
            ..PdfOptions::default()
        },
        PdfOptions {
            entry: "../index.typ".into(),
            ..PdfOptions::default()
        },
        PdfOptions {
            appendices: vec!["missing.typ".into()],
            ..PdfOptions::default()
        },
        PdfOptions {
            template: Some("../template.typ".into()),
            ..PdfOptions::default()
        },
        PdfOptions {
            front_cover: Some("../cover.typ".into()),
            ..PdfOptions::default()
        },
        PdfOptions {
            back_cover: Some("missing.typ".into()),
            ..PdfOptions::default()
        },
    ] {
        assert!(
            compiler(&project, runner.clone(), options, &output)
                .compile()
                .is_err()
        );
    }
    assert_eq!(runner.calls.load(Ordering::SeqCst), 0);
    project.source("lib/standalone.typ", "Print-only appendix");
    let options = PdfOptions {
        appendices: vec!["lib/standalone.typ".into()],
        ..PdfOptions::default()
    };
    assert!(
        compiler(&project, runner.clone(), options, &output)
            .compile()
            .is_ok()
    );
    assert_eq!(runner.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn appendix_order_and_print_options_are_part_of_the_cache_key() {
    let (project, runner, output) = fixture();
    project.source("a.typ", "Appendix A");
    project.source("b.typ", "Appendix B");
    let options = PdfOptions {
        appendices: vec!["b.typ".into(), "a.typ".into(), "b.typ".into()],
        ..PdfOptions::default()
    };
    compiler(&project, runner.clone(), options.clone(), &output)
        .compile()
        .unwrap();
    let first = fs::read_to_string(&output).unwrap();
    assert!(first.contains(r#"[\"/b\",\"/a\",\"/b\"]"#));
    let changed = PdfOptions {
        title: Some("Another document title".into()),
        ..options
    };
    assert!(
        compiler(&project, runner.clone(), changed, &output)
            .compile()
            .unwrap()
            .compiled
    );
    assert_eq!(runner.calls.load(Ordering::SeqCst), 2);
}

#[test]
fn shared_layout_paths_are_validated_and_changes_invalidate_the_book() {
    let (project, runner, output) = fixture();
    let original = fs::read_to_string(project.config.join("options.toml")).unwrap();
    for module in ["../outside.typ", "missing.typ", "C:/outside.typ"] {
        project.configure(
            "options.toml",
            &original.replace("module = \"\"", &format!("module = \"{module}\"")),
        );
        assert!(
            compiler(&project, runner.clone(), PdfOptions::default(), &output)
                .compile()
                .is_err()
        );
    }
    assert_eq!(runner.calls.load(Ordering::SeqCst), 0);
    // Layout modules outside typst_lib are still excluded from implicit articles.
    project.source(
        "page layout.typ",
        "#let page-header(info) = [Hello]\n#let page-footer(info) = []",
    );
    project.configure(
        "options.toml",
        &original.replace("module = \"\"", "module = \"page layout.typ\""),
    );
    let compiler = compiler(&project, runner.clone(), PdfOptions::default(), &output);
    compiler.compile().unwrap();
    let driver = fs::read_to_string(&output).unwrap();
    assert!(driver.contains("#import \"/page layout.typ\": page-header, page-footer"));
    assert!(!driver.contains("\"/page layout\": () => include"));
    assert!(!compiler.compile().unwrap().compiled);
    project.source(
        "page layout.typ",
        "#let page-header(info) = [Updated]\n#let page-footer(info) = []",
    );
    assert!(compiler.compile().unwrap().compiled);
    project.configure("options.toml", &original);
    assert!(compiler.compile().unwrap().compiled);
    assert!(
        !fs::read_to_string(output)
            .unwrap()
            .contains("#import \"/page layout.typ\"")
    );
}

#[test]
fn unavailable_offline_fonts_retain_the_last_good_book_without_compiling() {
    use typsite::compile::pdf::fonts::FontProfile;
    let (project, runner, output) = fixture();
    compiler(&project, runner.clone(), PdfOptions::default(), &output)
        .compile()
        .unwrap();
    let good = fs::read(&output).unwrap();
    let offline = compiler(
        &project,
        runner.clone(),
        PdfOptions {
            font_profile: FontProfile::NotoSerifSc,
            offline: true,
            ..PdfOptions::default()
        },
        &output,
    );
    assert!(
        offline
            .compile()
            .unwrap_err()
            .to_string()
            .contains("missing or corrupt")
    );
    assert_eq!(fs::read(&output).unwrap(), good);
    assert_eq!(runner.calls.load(Ordering::SeqCst), 1);
    let cached = compiler(
        &project,
        runner,
        PdfOptions {
            offline: true,
            ..PdfOptions::default()
        },
        &output,
    );
    assert!(
        !cached.compile().unwrap().compiled,
        "Offline is a transport policy, not a PDF content input"
    );
}

#[test]
fn pdf_cache_is_recognized_by_safe_cleanup() {
    let (project, runner, output) = fixture();
    compiler(&project, runner, PdfOptions::default(), &output)
        .compile()
        .unwrap();
    // An empty output root plus a PDF-only cache needs no unsafe --force.
    typsite::compile::clean::clean_roots(
        &project.cache,
        &project.output,
        &[&project.input, &project.config],
        false,
    )
    .unwrap();
    assert!(!project.cache.join("pdf").exists());
    assert!(
        output.exists(),
        "A standalone book is not an HTML-owned output"
    );
}

#[cfg(unix)]
#[test]
fn book_rejects_output_and_cache_artifact_symlinks() {
    use std::os::unix::fs::symlink;
    let (project, runner, output) = fixture();
    fs::create_dir_all(output.parent().unwrap()).unwrap();
    let victim = project.input.join("victim.pdf");
    fs::write(&victim, "Protected source").unwrap();
    symlink(&victim, &output).unwrap();
    assert!(
        PdfCompiler::with_runner(
            project.input.clone(),
            project.config.clone(),
            project.cache.clone(),
            output.clone(),
            None,
            PdfOptions::default(),
            runner.clone()
        )
        .is_err()
    );
    fs::remove_file(&output).unwrap();
    let compiler = compiler(&project, runner, PdfOptions::default(), &output);
    compiler.compile().unwrap();
    let artifact = artifact(&project, "pdf");
    fs::remove_file(&artifact).unwrap();
    symlink(&victim, &artifact).unwrap();
    assert!(compiler.compile().is_err());
    assert_eq!(fs::read_to_string(victim).unwrap(), "Protected source");
}
