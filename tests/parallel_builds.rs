mod common;

use anyhow::{Result, ensure};
use common::{FakeTypst, Project, clean_equivalent, html, successful};
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use typsite::compile::report::Stage;
use typsite::compile::typst::{ExportRequest, TypstRunner};

#[derive(Default)]
struct State {
    arrived: usize,
    active: usize,
    peak: usize,
}

#[derive(Default)]
struct ConcurrentRunner {
    state: Mutex<State>,
    ready: Condvar,
    prepares: AtomicUsize,
    calls: AtomicUsize,
    parallel: AtomicBool,
    fail: AtomicBool,
    edit_dependency: Mutex<Option<PathBuf>>,
}

impl TypstRunner for ConcurrentRunner {
    fn identity(&self) -> Result<String> {
        Ok("concurrent runner".into())
    }

    fn prepare(&self, _: Option<&Path>) -> Result<()> {
        assert_eq!(
            self.state.lock().unwrap().active,
            0,
            "prepare must not race exports"
        );
        self.prepares.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn export(&self, request: &ExportRequest<'_>) -> Result<Vec<PathBuf>> {
        assert!(self.prepares.load(Ordering::SeqCst) != 0);
        assert!(request.jobs.get() >= 1);
        self.calls.fetch_add(1, Ordering::SeqCst);
        {
            let mut state = self.state.lock().unwrap();
            state.arrived += 1;
            state.active += 1;
            state.peak = state.peak.max(state.active);
            self.ready.notify_all();
            // The first pair cannot complete unless two exports really overlap.
            // A bounded wait fails rather than hanging the suite on regressions.
            if self.parallel.load(Ordering::SeqCst) && state.arrived < 2 {
                let (next, timeout) = self
                    .ready
                    .wait_timeout_while(state, Duration::from_secs(5), |state| state.arrived < 2)
                    .unwrap();
                state = next;
                assert!(
                    !timeout.timed_out(),
                    "article exports did not run concurrently"
                );
            }
            drop(state);
        }
        // Vary completion order; reports and publication must still be ordered.
        let name = request.input.file_name().unwrap().to_str().unwrap();
        std::thread::sleep(Duration::from_millis(if name == "a.typ" { 15 } else { 3 }));
        let dependency = request.root.join("lib/shared.typ");
        let dependencies = if dependency.exists() {
            vec![request.input.to_owned(), dependency.clone()]
        } else {
            vec![request.input.to_owned()]
        };
        let mut body = fs::read_to_string(request.input)?;
        if dependency.exists() {
            body = body.replace("{{include}}", &fs::read_to_string(&dependency)?);
        }
        if let Some(path) = self.edit_dependency.lock().unwrap().take() {
            fs::write(path, "changed during parallel export")?;
        }
        fs::write(request.output, &body)?;
        self.state.lock().unwrap().active -= 1;
        ensure!(
            !(self.fail.load(Ordering::SeqCst) && matches!(name, "a.typ" | "c.typ")),
            "failed export for {name}"
        );
        Ok(dependencies)
    }
}

fn runner(parallel: bool) -> Arc<ConcurrentRunner> {
    Arc::new(ConcurrentRunner {
        parallel: AtomicBool::new(parallel),
        ..ConcurrentRunner::default()
    })
}

fn two_threads<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap()
        .install(f)
}

#[test]
fn exports_overlap_with_a_bounded_worker_count_and_match_sequential_bytes() {
    two_threads(|| {
        let project = Project::new();
        for name in ["a", "b", "c", "d"] {
            project.source(&format!("{name}.typ"), &html(name));
        }
        let runner = runner(true);
        let compiler = project
            .compiler(runner.clone())
            .with_jobs(NonZeroUsize::new(2).unwrap());
        let report = successful(&compiler);
        assert_eq!(
            report.exported,
            ["a.typ", "b.typ", "c.typ", "d.typ"].map(PathBuf::from)
        );
        assert_eq!(runner.prepares.load(Ordering::SeqCst), 1);
        assert_eq!(runner.state.lock().unwrap().peak, 2);
        let calls = runner.calls.load(Ordering::SeqCst);
        let sequential = project
            .compiler(runner.clone())
            .with_jobs(NonZeroUsize::MIN);
        assert!(
            !successful(&sequential).changed(),
            "changing jobs must not invalidate caches"
        );
        assert_eq!(runner.calls.load(Ordering::SeqCst), calls);
        let parallel_bytes = project.snapshot();
        runner.parallel.store(false, Ordering::SeqCst);
        runner.state.lock().unwrap().peak = 0;
        fs::remove_dir_all(&project.cache).unwrap();
        fs::remove_dir_all(&project.output).unwrap();
        successful(&sequential);
        assert_eq!(
            runner.state.lock().unwrap().peak,
            1,
            "--jobs 1 must serialize exports"
        );
        assert_eq!(parallel_bytes, project.snapshot());
        assert_eq!(runner.prepares.load(Ordering::SeqCst), 2);
    });
}

#[test]
fn failed_workers_keep_good_artifacts_and_last_good_pages_and_retry_only_failures() {
    two_threads(|| {
        let project = Project::new();
        for name in ["a", "b", "c", "d"] {
            project.source(&format!("{name}.typ"), &html(name));
        }
        let runner = runner(true);
        let compiler = project
            .compiler(runner.clone())
            .with_jobs(NonZeroUsize::new(2).unwrap());
        successful(&compiler);
        let published = project.snapshot();
        let a_artifact = fs::read(project.cache.join("exports/a.html")).unwrap();
        for name in ["a", "b", "c", "d"] {
            project.source(&format!("{name}.typ"), &html(&format!("changed {name}")));
        }
        runner.fail.store(true, Ordering::SeqCst);
        let report = compiler.compile().unwrap();
        assert!(!report.is_success());
        assert_eq!(
            runner.state.lock().unwrap().active,
            0,
            "all workers must join before returning"
        );
        assert_eq!(report.exported, ["b.typ", "d.typ"].map(PathBuf::from));
        let failures: Vec<_> = report
            .diagnostics
            .iter()
            .map(|diagnostic| {
                assert_eq!(diagnostic.stage, Stage::Export);
                diagnostic.path.clone().unwrap()
            })
            .collect();
        assert_eq!(failures, ["a.typ", "c.typ"].map(PathBuf::from));
        assert_eq!(published, project.snapshot());
        assert_eq!(
            a_artifact,
            fs::read(project.cache.join("exports/a.html")).unwrap()
        );
        assert!(
            fs::read_to_string(project.cache.join("exports/b.html"))
                .unwrap()
                .contains("changed b")
        );
        runner.fail.store(false, Ordering::SeqCst);
        assert_eq!(
            successful(&compiler).exported,
            ["a.typ", "c.typ"].map(PathBuf::from)
        );
        clean_equivalent(&project, &compiler);
    });
}

#[test]
fn concurrent_exports_reject_racing_shared_import_generations_and_recover() {
    two_threads(|| {
        let project = Project::new();
        project.source("a.typ", &html("{{include}}"));
        project.source("b.typ", &html("{{include}}"));
        project.source("lib/shared.typ", "original dependency");
        let runner = runner(true);
        *runner.edit_dependency.lock().unwrap() = Some(project.input.join("lib/shared.typ"));
        let compiler = project
            .compiler(runner)
            .with_jobs(NonZeroUsize::new(2).unwrap());
        assert!(!compiler.compile().unwrap().is_success());
        assert!(!project.output.join("a.html").exists());
        successful(&compiler);
        for name in ["a.html", "b.html"] {
            assert!(
                fs::read_to_string(project.output.join(name))
                    .unwrap()
                    .contains("changed during parallel export")
            );
        }
        clean_equivalent(&project, &compiler);
    });
}

#[test]
fn conflicting_export_targets_are_rejected_before_workers_start() {
    // a.html is a directory in the source tree but an article's export target.
    // Reject before workers can race creating that directory and replacing it.
    let project = Project::new();
    project.source("a.typ", &html("A"));
    project.source("a.html/nested.typ", &html("Nested"));
    let runner = Arc::new(FakeTypst::default());
    let compiler = project
        .compiler(runner.clone())
        .with_jobs(NonZeroUsize::new(2).unwrap());
    assert!(
        compiler
            .compile()
            .unwrap_err()
            .to_string()
            .contains("collision")
    );
    assert!(runner.calls().is_empty());
    assert!(!project.cache.join("exports/a.html").exists());
}

#[test]
fn parallel_parse_diagnostics_have_stable_source_order() {
    two_threads(|| {
        let project = Project::new();
        for name in ["c", "a", "b"] {
            project.source(&format!("{name}.typ"), "not valid exported HTML");
        }
        let compiler = project.compiler(Arc::new(FakeTypst::default()));
        let report = compiler.compile().unwrap();
        assert!(!report.is_success());
        assert_eq!(
            report
                .diagnostics
                .iter()
                .map(|diagnostic| {
                    assert_eq!(diagnostic.stage, Stage::Parse);
                    diagnostic.path.clone().unwrap()
                })
                .collect::<Vec<_>>(),
            ["a.typ", "b.typ", "c.typ"].map(PathBuf::from)
        );
    });
}
