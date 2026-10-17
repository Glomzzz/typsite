mod common;

use common::{FakeTypst, Project, html, successful};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use typsite::compile::clean::clean_roots;

fn command(project: &Project) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_typsite"));
    command
        .current_dir(&project.root)
        .env(
            "TYPST_PACKAGE_PATH",
            project.root.join("isolated namespace"),
        )
        .env(
            "TYPST_PACKAGE_CACHE_PATH",
            project.root.join("isolated downloads"),
        )
        .env("TYPSITE_TEST_EXPORT_LOG", project.root.join("export.log"));
    command
}

fn stub(project: &Project) -> PathBuf {
    let executable = project
        .root
        .join(format!("typst test stub{}", std::env::consts::EXE_SUFFIX));
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg("--edition=2021")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/typst_stub.rs"))
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    executable
}

#[test]
fn cli_accepts_absolute_custom_roots_and_a_typst_executable_with_spaces() {
    let project = Project::named("typsite cli [paths with spaces]");
    project.source("nested/a.typ", &html("CLI content"));
    let executable = stub(&project);
    let compile = || {
        command(&project)
            .arg("compile")
            .arg("--input")
            .arg(&project.input)
            .arg("--config")
            .arg(&project.config)
            .arg("--cache")
            .arg(&project.cache)
            .arg("--output")
            .arg(&project.output)
            .arg("--typst")
            .arg(std::path::Path::new(".").join(executable.file_name().unwrap()))
            .output()
            .unwrap()
    };
    let cold = compile();
    assert!(
        cold.status.success(),
        "{}",
        String::from_utf8_lossy(&cold.stderr)
    );
    assert!(
        fs::read_to_string(project.output.join("nested/a.html"))
            .unwrap()
            .contains("CLI content")
    );
    let modified = fs::metadata(project.output.join("nested/a.html"))
        .unwrap()
        .modified()
        .unwrap();
    let warm = compile();
    assert!(
        warm.status.success(),
        "{}",
        String::from_utf8_lossy(&warm.stderr)
    );
    assert!(
        String::from_utf8_lossy(&warm.stdout)
            .contains("0 exported, 0 rendered, 0 written, 0 removed")
    );
    assert_eq!(
        fs::read_to_string(project.root.join("export.log"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert_eq!(
        modified,
        fs::metadata(project.output.join("nested/a.html"))
            .unwrap()
            .modified()
            .unwrap()
    );
}

#[test]
fn cli_publication_errors_exit_nonzero_and_an_ordinary_rerun_repairs_them() {
    let project = Project::new();
    project.source("a.typ", &html("CLI publication"));
    project.write(project.output.join("a.html/blocking-file"), "blocked");
    let executable = stub(&project);
    let compile = || {
        command(&project)
            .args(["c", "--input", "content", "--cache", "cache"])
            .arg("--typst")
            .arg(&executable)
            .output()
            .unwrap()
    };
    let failed = compile();
    assert!(!failed.status.success());
    assert!(
        String::from_utf8_lossy(&failed.stderr).contains("Publish"),
        "{}",
        String::from_utf8_lossy(&failed.stderr)
    );
    fs::remove_dir_all(project.output.join("a.html")).unwrap();
    let repaired = compile();
    assert!(
        repaired.status.success(),
        "{}",
        String::from_utf8_lossy(&repaired.stderr)
    );
    assert!(project.output.join("a.html").is_file());
    assert_eq!(
        fs::read_to_string(project.root.join("export.log"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[test]
fn clean_validates_every_root_before_deleting_anything() {
    let project = Project::new();
    project.source("a.typ", &html("A"));
    let compiler = project.compiler(Arc::new(FakeTypst::default()));
    successful(&compiler);
    assert!(
        clean_roots(
            &project.cache,
            &project.input,
            &[&project.input, &project.config],
            true
        )
        .is_err()
    );
    assert!(project.cache.join("manifest.json").exists());
    assert!(project.input.join("a.typ").exists());
    assert!(clean_roots(&project.cache, &project.cache.join("nested"), &[], true).is_err());
    assert!(clean_roots(&project.cache, &std::env::current_dir().unwrap(), &[], true).is_err());
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(project.output.join(".typsite.lock"))
        .unwrap();
    lock.lock().unwrap();
    assert!(compiler.clean().is_err());
    assert!(project.cache.join("manifest.json").exists());
    lock.unlock().unwrap();
    compiler.clean().unwrap();
    assert!(project.output.join(".typsite.lock").exists());
    assert!(!project.output.join("a.html").exists());
    assert!(!project.cache.join("manifest.json").exists());
    successful(&compiler);
}

#[test]
fn cleaning_unmarked_legacy_directories_requires_explicit_force() {
    let project = Project::new();
    project.write(project.cache.join("legacy-cache"), "disposable");
    let clean = |force: bool| {
        let mut command = command(&project);
        command.args(["clean", "--cache", "cache", "--input", "content"]);
        if force {
            command.arg("--force");
        }
        command.output().unwrap()
    };
    let refused = clean(false);
    assert!(!refused.status.success());
    assert!(project.cache.join("legacy-cache").exists());
    let forced = clean(true);
    assert!(
        forced.status.success(),
        "{}",
        String::from_utf8_lossy(&forced.stderr)
    );
    assert!(!project.cache.join("legacy-cache").exists());
    assert!(project.config.join("options.toml").exists());
}

#[cfg(unix)]
#[test]
fn watch_broadcasts_changes_preserves_last_good_pages_and_shuts_down() {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::time::{Duration, Instant};
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let project = Project::new();
    project.source("a.typ", &html("Original"));
    let executable = stub(&project);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut child = Child(
        command(&project)
            .args([
                "c",
                "--input",
                "content",
                "--cache",
                "cache",
                "--host",
                "127.0.0.1",
                "--port",
            ])
            .arg(address.port().to_string())
            .arg("--typst")
            .arg(executable)
            .stdout(fs::File::create(project.root.join("watch.stdout")).unwrap())
            .stderr(fs::File::create(project.root.join("watch.stderr")).unwrap())
            .spawn()
            .unwrap(),
    );
    let get = |path: &str| -> std::io::Result<String> {
        let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(100))?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )?;
        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        Ok(response)
    };
    let revision = || {
        get("/_reload").ok().and_then(|response| {
            response
                .split_once("\r\n\r\n")
                .and_then(|(_, body)| body.trim().parse::<u64>().ok())
        })
    };
    let await_revision = |minimum: u64| {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(revision) = revision().filter(|revision| *revision >= minimum) {
                break revision;
            }
            assert!(
                Instant::now() < deadline,
                "watch failed: {}",
                fs::read_to_string(project.root.join("watch.stderr")).unwrap()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    let initial = await_revision(1);
    assert_eq!(revision(), Some(initial));
    assert_eq!(
        revision(),
        Some(initial),
        "another client must not consume the revision"
    );
    std::thread::sleep(Duration::from_millis(1100));
    assert_eq!(revision(), Some(initial), "no-op polls must not reload");
    project.source("a.typ", &html("Changed"));
    let changed = await_revision(initial + 1);
    assert!(get("/a?query=yes").unwrap().contains("Changed"));
    let schema = fs::read_to_string(project.config.join("schemas/main.html")).unwrap();
    project.configure("schemas/main.html", "broken template");
    project.source("a.typ", &html("Fixed"));
    std::thread::sleep(Duration::from_millis(1100));
    assert_eq!(revision(), Some(changed));
    assert!(get("/a").unwrap().contains("Changed"));
    project.configure("schemas/main.html", &schema);
    await_revision(changed + 1);
    assert!(get("/a").unwrap().contains("Fixed"));
    assert!(
        Command::new("kill")
            .args(["-INT", &child.0.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "watch did not shut down gracefully"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(unix)]
#[test]
fn parent_components_are_resolved_after_symlinks_not_before_them() {
    let project = Project::new();
    fs::create_dir_all(project.input.join("nested")).unwrap();
    let alias = project.root.join("alias");
    std::os::unix::fs::symlink(project.input.join("nested"), &alias).unwrap();
    let resolved = typsite::compile::project::resolve(&alias.join("..")).unwrap();
    assert_eq!(resolved, fs::canonicalize(&project.input).unwrap());
    assert!(
        clean_roots(
            &project.cache,
            &alias.join("../publish"),
            &[&project.input],
            true
        )
        .is_err()
    );
}
