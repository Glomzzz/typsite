//! Preview adapters. Blocking HTTP and build work never run on Tokio workers.

use super::compiler::Compiler;
use super::project::{managed_path, validate_relative};
use super::report::BuildReport;
use anyhow::{Context, Result, anyhow, bail, ensure};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tiny_http::{Header, Method, Request, Response, Server};

const REVISION_MARKER: &str = "__TYPSITE_RELOAD_REVISION__";
pub const WATCH_AUTO_RELOAD_SCRIPT: &str = r#"
<script>
(() => {
  const revision = "__TYPSITE_RELOAD_REVISION__";
  let polling = false;
  setInterval(async () => {
    if (polling) return;
    polling = true;
    try {
      const response = await fetch('/_reload', { cache: 'no-store' });
      if (response.ok && (await response.text()).trim() !== revision) location.reload();
    } catch (_) { /* The server may be restarting. Retry on the next tick. */ }
    finally { polling = false; }
  }, 500);
})();
</script>
"#;

pub async fn watch(compiler: Compiler, host: String, port: u16) -> Result<()> {
    let server =
        Arc::new(Server::http(format!("{host}:{port}")).map_err(|error| anyhow!("{error}"))?);
    println!("  - Serve url: http://{}", server.server_addr());
    let root = compiler.output_path().to_owned();
    let compiler = Arc::new(compiler);
    let stopped = Arc::new(AtomicBool::new(false));
    let revision = Arc::new(AtomicU64::new(0));
    let mut http = {
        let server = server.clone();
        let stopped = stopped.clone();
        let revision = revision.clone();
        tokio::task::spawn_blocking(move || serve(&server, &root, &revision, &stopped))
    };
    let mut builds = {
        let stopped = stopped.clone();
        let revision = revision.clone();
        tokio::spawn(async move {
            while !stopped.load(Ordering::Acquire) {
                let compiler = compiler.clone();
                match tokio::task::spawn_blocking(move || compiler.compile())
                    .await
                    .context("Build task panicked")?
                {
                    Ok(report) => {
                        print_report(&report);
                        if report.is_success() && report.changed() {
                            revision.fetch_add(1, Ordering::AcqRel);
                        }
                    }
                    Err(error) => eprintln!("Build failed: {error:#}"),
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Ok::<(), anyhow::Error>(())
        })
    };
    enum Finished {
        Signal(std::io::Result<()>),
        Http(Result<Result<()>, tokio::task::JoinError>),
        Build(Result<Result<()>, tokio::task::JoinError>),
    }
    let finished = tokio::select! {
        result = tokio::signal::ctrl_c() => Finished::Signal(result),
        result = &mut http => Finished::Http(result),
        result = &mut builds => Finished::Build(result),
    };
    stopped.store(true, Ordering::Release);
    server.unblock();
    // Finish any in-flight export rather than abandoning a child process or a
    // publication halfway through. Build locks are released before returning.
    match finished {
        Finished::Signal(signal) => {
            let (builds, http) = tokio::join!(builds, http);
            builds.context("Build task panicked")??;
            http.context("Preview server task panicked")??;
            signal.context("Cannot listen for shutdown")?;
            Ok(())
        }
        Finished::Http(result) => {
            builds.await.context("Build task panicked")??;
            result.context("Preview server task panicked")??;
            bail!("Preview server stopped unexpectedly")
        }
        Finished::Build(result) => {
            http.await.context("Preview server task panicked")??;
            result.context("Build task panicked")??;
            bail!("Build task stopped unexpectedly")
        }
    }
}

pub(crate) fn print_report(report: &BuildReport) {
    for diagnostic in &report.diagnostics {
        eprintln!(
            "{:?} {:?}: {}",
            diagnostic.stage, diagnostic.path, diagnostic.message
        );
    }
    if report.is_success() && report.changed() {
        println!(
            "Built: {} exported, {} rendered, {} written, {} removed",
            report.exported.len(),
            report.rendered,
            report.written.len(),
            report.removed.len()
        );
    }
}

fn serve(server: &Server, root: &Path, revision: &AtomicU64, stopped: &AtomicBool) -> Result<()> {
    while !stopped.load(Ordering::Acquire) {
        if let Some(request) = server
            .recv_timeout(Duration::from_millis(100))
            .context("Preview server receive failed")?
        {
            if let Err(error) = respond(request, root, revision.load(Ordering::Acquire)) {
                // A disconnected browser must not stop the server.
                eprintln!("[WARN] Preview response failed: {error:#}");
            }
        }
    }
    Ok(())
}

fn respond(request: Request, root: &Path, revision: u64) -> Result<()> {
    if request.method() != &Method::Get && request.method() != &Method::Head {
        request.respond(Response::empty(405).with_header(header("Allow", "GET, HEAD")))?;
        return Ok(());
    }
    let path = match request_path(request.url()) {
        Ok(path) => path,
        Err(_) => {
            request.respond(Response::empty(403))?;
            return Ok(());
        }
    };
    if path == Path::new("_reload") {
        request.respond(
            Response::from_string(revision.to_string())
                .with_header(header("Cache-Control", "no-store"))
                .with_header(header("Content-Type", "text/plain; charset=utf-8")),
        )?;
        return Ok(());
    }
    let path = match route(root, &path) {
        Ok(Some(path)) => path,
        Ok(None) => {
            request.respond(Response::empty(404))?;
            return Ok(());
        }
        Err(_) => {
            request.respond(Response::empty(403))?;
            return Ok(());
        }
    };
    let content_type = mime_type(&path);
    let cache = header("Cache-Control", "no-store");
    let mime = header("Content-Type", content_type);
    if path.extension().is_some_and(|ext| ext == "html") {
        let mut html = std::fs::read_to_string(&path)?;
        // Also instrument a last-good production build while a broken project
        // is being repaired in watch mode. Only served bytes are modified.
        if !html.contains(REVISION_MARKER) {
            if let Some(index) = html.rfind("</body>") {
                html.insert_str(index, WATCH_AUTO_RELOAD_SCRIPT);
            } else {
                html.push_str(WATCH_AUTO_RELOAD_SCRIPT);
            }
        }
        html = html.replace(REVISION_MARKER, &revision.to_string());
        request.respond(
            Response::from_string(html)
                .with_header(cache)
                .with_header(mime),
        )?;
    } else {
        match File::open(&path) {
            Ok(file) => request.respond(
                Response::from_file(file)
                    .with_header(cache)
                    .with_header(mime),
            )?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                request.respond(Response::empty(404))?
            }
            Err(error) => {
                request.respond(Response::empty(500))?;
                return Err(error.into());
            }
        }
    }
    Ok(())
}

fn request_path(url: &str) -> Result<PathBuf> {
    let raw = url.split('?').next().unwrap_or(url);
    ensure!(raw.starts_with('/'), "Not an absolute request path");
    let mut bytes = Vec::new();
    let mut input = raw.bytes();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let high = input.next().context("Incomplete URL escape")?;
            let low = input.next().context("Incomplete URL escape")?;
            let high = (high as char).to_digit(16).context("Invalid URL escape")?;
            let low = (low as char).to_digit(16).context("Invalid URL escape")?;
            bytes.push((high * 16 + low) as u8);
        } else {
            bytes.push(byte);
        }
    }
    let decoded = String::from_utf8(bytes).context("Request URL is not UTF-8")?;
    let relative = decoded.trim_start_matches('/');
    ensure!(!relative.contains('\\'), "Backslash in URL");
    for part in relative.split('/') {
        ensure!(
            part != "." && part != ".." && !part.starts_with(".typsite"),
            "Private or traversing request path"
        );
    }
    if relative.is_empty() {
        return Ok(PathBuf::new());
    }
    let path = PathBuf::from(relative);
    validate_relative(&path)?;
    Ok(path)
}

fn route(root: &Path, path: &Path) -> Result<Option<PathBuf>> {
    let candidates = if path.as_os_str().is_empty() {
        vec![PathBuf::from("index.html")]
    } else {
        vec![
            path.to_owned(),
            PathBuf::from(format!("{}.html", path.display())),
            path.join("index.html"),
        ]
    };
    for relative in candidates {
        let candidate = managed_path(root, &relative)?;
        if candidate.is_file() {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name, value).expect("static HTTP headers must be valid")
}

fn mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "xml" => "application/xml; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "txt" => "text/plain; charset=utf-8",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::thread;

    struct Preview {
        root: PathBuf,
        server: Arc<Server>,
        revision: Arc<AtomicU64>,
        stopped: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<Result<()>>>,
    }
    impl Preview {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "typsite-http-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(root.join("nested")).unwrap();
            std::fs::write(root.join("index.html"), "<body>Home</body>").unwrap();
            std::fs::write(root.join("nested/index.html"), "<body>Nested</body>").unwrap();
            std::fs::write(root.join("page.html"), "<body>Page</body>").unwrap();
            std::fs::write(root.join("v0.15.html"), "<body>Version</body>").unwrap();
            std::fs::write(root.join("style.css"), "body {}").unwrap();
            let server = Arc::new(Server::http("127.0.0.1:0").unwrap());
            let revision = Arc::new(AtomicU64::new(7));
            let stopped = Arc::new(AtomicBool::new(false));
            let thread = {
                let (root, server, revision, stopped) = (
                    root.clone(),
                    server.clone(),
                    revision.clone(),
                    stopped.clone(),
                );
                thread::spawn(move || serve(&server, &root, &revision, &stopped))
            };
            Self {
                root,
                server,
                revision,
                stopped,
                thread: Some(thread),
            }
        }
        fn request(&self, method: &str, path: &str) -> String {
            let mut stream = TcpStream::connect(self.server.server_addr().to_string()).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            write!(
                stream,
                "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            response
        }
    }
    impl Drop for Preview {
        fn drop(&mut self) {
            self.stopped.store(true, Ordering::Release);
            self.server.unblock();
            self.thread.take().unwrap().join().unwrap().unwrap();
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn revisions_are_broadcast_and_each_page_gets_its_load_revision() {
        let preview = Preview::new();
        for _ in 0..3 {
            assert!(preview.request("GET", "/_reload").ends_with("7"));
        }
        let page = preview.request("GET", "/");
        assert!(page.contains("const revision = \"7\""));
        assert!(!page.contains(REVISION_MARKER));
        preview.revision.store(8, Ordering::Release);
        for _ in 0..3 {
            assert!(preview.request("GET", "/_reload").ends_with("8"));
        }
        assert!(
            !std::fs::read_to_string(preview.root.join("index.html"))
                .unwrap()
                .contains("<script>")
        );
    }

    #[test]
    fn urls_pretty_paths_queries_mime_types_and_head_requests_work() {
        let preview = Preview::new();
        for path in ["/", "/page", "/page.html?query=yes", "/nested/", "/v0.15"] {
            assert!(
                preview.request("GET", path).starts_with("HTTP/1.1 200"),
                "{path}"
            );
        }
        let css = preview.request("GET", "/style.css?v=1");
        assert!(css.contains("text/css"));
        assert!(css.ends_with("body {}"));
        let head = preview.request("HEAD", "/page");
        assert!(head.starts_with("HTTP/1.1 200"));
        assert!(!head.contains("<body>Page"));
        assert!(preview.request("POST", "/page").starts_with("HTTP/1.1 405"));
        assert!(
            preview
                .request("GET", "/missing")
                .starts_with("HTTP/1.1 404")
        );
    }

    #[test]
    fn traversal_and_private_manifest_requests_are_rejected() {
        let preview = Preview::new();
        for path in [
            "/../secret",
            "/%2e%2e/secret",
            "/x/%2e%2e/secret",
            "/%5c..%5csecret",
            "/.typsite.lock",
            "/.typsite-outputs.json",
            "/%00",
            "/%xy",
        ] {
            assert!(
                preview.request("GET", path).starts_with("HTTP/1.1 403"),
                "{path}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_output_paths_are_not_served() {
        let preview = Preview::new();
        std::os::unix::fs::symlink(&preview.root, preview.root.join("escape")).unwrap();
        assert!(
            preview
                .request("GET", "/escape/index.html")
                .starts_with("HTTP/1.1 403")
        );
    }

    #[test]
    fn disconnected_clients_do_not_stop_the_server() {
        let preview = Preview::new();
        let mut stream = TcpStream::connect(preview.server.server_addr().to_string()).unwrap();
        stream
            .write_all(b"GET /missing HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        drop(stream);
        assert!(preview.request("GET", "/page").starts_with("HTTP/1.1 200"));
    }
}
