use crate::util::error::log_err;
use anyhow::*;
use std::path::PathBuf;
use std::result::Result::Ok;
use std::{fs::File, path::Path, time::Duration};
use tiny_http::{Response, Server};
use tokio::sync::mpsc::{self, Receiver, Sender};

use super::compiler::Compiler;

pub const WATCH_AUTO_RELOAD_SCRIPT: &str = r"
<script>
    setInterval(async () => {
      const res = await fetch('/_reload')
      if (await res.text() === '1') {
        location.reload()
      }
    }, 500)
  </script>
";

//noinspection ALL
pub async fn watch(compiler: Compiler,host:String, port: u16) -> Result<()> {
    let (reload_sender, reload_receiver) = mpsc::channel::<()>(1);

    let url = format!("{host}:{port}");
    println!("  - Serve url: http://{url}");
    let server_result = Server::http(url);
    if let Err(e) = server_result {
        return Err(anyhow!("{e}"));
    }
    let server = match server_result {
        Ok(server) => server,
        Err(err) => return Err(anyhow!(err)),
    };
    let publish_dir = compiler.output_path().to_path_buf();

    let server_task = tokio::task::Builder::new()
        .name("server_task")
        .spawn(async move {
            server_task(server, publish_dir, reload_receiver);
        })
        .context("Failed to spawn watch_task")?;

    let compile_task = tokio::task::Builder::new()
        .name("compile_task")
        .spawn(async move {
            compile_task(compiler, reload_sender).await;
        })
        .context("Failed to spawn compile_task")?;

    let _ = tokio::join!(compile_task, server_task);

    Ok(())
}

async fn compile_task(compiler: Compiler, reload_sender: Sender<()>) {
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let result = compiler.compile();
        if let Ok((true,_)) = result {
            let _ = reload_sender.try_send(());
        } else {
            log_err(result);
        }
    }
}

fn server_task(server: Server, publish_dir: PathBuf, mut reload_receiver: Receiver<()>) {
    for request in server.incoming_requests() {
        let raw_path = request.url().trim_start_matches('/');

        if raw_path == "_reload" {
            let refresh = reload_receiver.try_recv().is_ok();
            let response = Response::from_string(if refresh { "1" } else { "0" });
            if let Err(err) = request.respond(response) {
                eprintln!("[WARN] Failed to respond to reload request: {err}");
            }
            continue;
        }

        if raw_path.contains("..") || raw_path.starts_with('/') {
            respond_403(request);
            continue;
        }

        let path = if raw_path.is_empty() {
            "index.html".to_string()
        } else if raw_path.contains(".") {
            raw_path.to_string()
        } else if raw_path.ends_with('/') {
            format!("{raw_path}/index.html")
        } else {
            format!("{raw_path}.html")
        };

        let full_path = publish_dir.join(&path);

        let raw_path_display = raw_path.to_string();
        let full_path_display = full_path.display();
        match File::open(Path::new(&full_path)) {
            Ok(file) => {
                let response = Response::from_file(file);
                println!("Request: {raw_path_display} -> {full_path_display}");
                if let Err(err) = request.respond(response) {
                    eprintln!("[WARN] Failed to respond with file {full_path_display}: {err}");
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!("Request: {raw_path_display} -> 404");
                respond_404(request);
            }
            _ => {
                println!("Request: {raw_path_display} -> 500");
                respond_500(request);
            }
        }
    }
}

fn respond_403(r: tiny_http::Request) {
    if let Err(err) = r.respond(Response::empty(403)) {
        eprintln!("[WARN] Failed to respond with 403: {err}");
    }
}
fn respond_404(r: tiny_http::Request) {
    if let Err(err) = r.respond(Response::empty(404)) {
        eprintln!("[WARN] Failed to respond with 404: {err}");
    }
}
fn respond_500(r: tiny_http::Request) {
    if let Err(err) = r.respond(Response::empty(500)) {
        eprintln!("[WARN] Failed to respond with 500: {err}");
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpStream;
    use std::thread;

    pub(crate) fn run_watch_response_errors_are_logged_not_panicked() {
        let server = Server::http("127.0.0.1:0").expect("server should bind to a local port");
        let addr = server.server_addr().to_string();
        let handle = thread::spawn(move || {
            let request = server.recv().expect("request should arrive");
            respond_404(request);
        });

        let mut stream = TcpStream::connect(&addr).expect("client should connect");
        stream
            .write_all(b"GET /missing HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("request should be written");
        drop(stream);

        handle.join().expect("respond_404 path should not panic");
    }

    #[test]
    fn watch_response_errors_are_logged_not_panicked() {
        run_watch_response_errors_are_logged_not_panicked();
    }
}
