//! The only boundary that invokes the Typst executable.

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::ffi::OsString;
use std::io::Write;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct ExportRequest<'a> {
    pub root: &'a Path,
    pub input: &'a Path,
    pub output: &'a Path,
    pub font_path: &'a Path,
    /// Validated source-root-anchored shared page layout module.
    pub layout: Option<&'a str>,
    /// Worker budget inside this Typst subprocess (not concurrent articles).
    pub jobs: NonZeroUsize,
}

/// A single native book, compiled from a generated driver without writing into
/// the user's source tree. All source loaders remain inside `root`.
pub struct PdfRequest<'a> {
    pub root: &'a Path,
    pub driver: &'a str,
    pub output: &'a Path,
    pub font_path: &'a Path,
    pub jobs: NonZeroUsize,
}

#[derive(Clone, Copy)]
enum Format {
    Html,
    Pdf,
}

/// The HTML `export` operation writes `output` and returns the files actually
/// read by Typst. `export` may be called concurrently for distinct output artifacts;
/// `prepare` always completes before any exports in that build. This small seam
/// lets build tests run without a shell or package installs.
pub trait TypstRunner: Send + Sync {
    fn identity(&self) -> Result<String>;
    /// Prepare package resources in the same environment used by `export`.
    /// Test runners need no filesystem installation side effects.
    /// Return installed resource directories to fingerprint before export.
    /// Unknown dependencies discovered by Typst still require verification.
    fn prepare(&self, _packages: Option<&Path>) -> Result<Vec<PathBuf>> {
        Ok(Vec::new())
    }
    fn export(&self, request: &ExportRequest<'_>) -> Result<Vec<PathBuf>>;
    /// Write one PDF and return its actual read set. Existing HTML-only runners
    /// need not implement books. Both operations share preparation and identity.
    fn pdf(&self, _request: &PdfRequest<'_>) -> Result<Vec<PathBuf>> {
        anyhow::bail!("This Typst runner does not support PDF books")
    }
}

pub struct CommandTypst {
    executable: PathBuf,
    package_path: Option<PathBuf>,
}

impl CommandTypst {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            package_path: None,
        }
    }

    /// Useful for hermetic builds: this is Typst's namespace root, containing
    /// e.g. `local/typsite/0.1.0`, not Typsite's package-source directory.
    pub fn with_package_path(mut self, path: PathBuf) -> Self {
        self.package_path = Some(path);
        self
    }

    fn executable(&self) -> Result<PathBuf> {
        if !self.executable.is_absolute()
            && self
                .executable
                .parent()
                .is_some_and(|parent| !parent.as_os_str().is_empty())
        {
            Ok(std::path::absolute(&self.executable)?)
        } else {
            Ok(self.executable.clone())
        }
    }

    fn environment() -> Result<Vec<(&'static str, Option<OsString>)>> {
        [
            "TYPST_FONT_PATHS",
            "TYPST_IGNORE_SYSTEM_FONTS",
            "TYPST_IGNORE_EMBEDDED_FONTS",
            "TYPST_PACKAGE_PATH",
            "TYPST_PACKAGE_CACHE_PATH",
            "SOURCE_DATE_EPOCH",
        ]
        .into_iter()
        .map(|key| {
            let value = std::env::var_os(key)
                .map(|value| absolute_environment_value(key, value))
                .transpose()?;
            Ok((key, value))
        })
        .collect()
    }

    fn command(&self, request: &ExportRequest<'_>, dependencies: &Path) -> Result<Command> {
        let mut command = self.compile_command(
            request.root,
            Some(request.input),
            request.output,
            request.font_path,
            request.jobs,
            dependencies,
            Format::Html,
        )?;
        if let Some(layout) = request.layout {
            let source = request.input.strip_prefix(request.root)?.with_extension("");
            command
                .arg("--input")
                .arg(format!("typsite-layout={layout}"))
                .arg("--input")
                .arg(format!(
                    "typsite-source=/{}",
                    source.to_string_lossy().replace('\\', "/")
                ));
        }
        Ok(command)
    }

    fn compile_command(
        &self,
        root: &Path,
        input: Option<&Path>,
        output: &Path,
        fonts: &Path,
        jobs: NonZeroUsize,
        dependencies: &Path,
        format: Format,
    ) -> Result<Command> {
        let mut command = Command::new(self.executable()?);
        for (key, value) in Self::environment()? {
            if let Some(value) = value {
                command.env(key, value);
            } else {
                command.env_remove(key);
            }
        }
        command
            .current_dir(std::path::absolute(root)?)
            .arg("compile")
            .arg(match input {
                Some(input) => std::path::absolute(input)?.into_os_string(),
                None => "-".into(),
            })
            .arg(std::path::absolute(output)?)
            .arg("--root")
            .arg(std::path::absolute(root)?);
        match format {
            Format::Html => {
                command.args([
                    "--format",
                    "html",
                    "--features",
                    "html",
                    "--input",
                    "html-frames=true",
                ]);
            }
            Format::Pdf => {
                command.args(["--format", "pdf", "--input", "typsite-pdf=true"]);
            }
        }
        command
            .arg("--font-path")
            .arg(std::path::absolute(fonts)?)
            .arg("--deps")
            .arg(std::path::absolute(dependencies)?)
            .args(["--deps-format", "json"])
            .arg("--jobs")
            .arg(jobs.to_string());
        if let Some(packages) = &self.package_path {
            command
                .arg("--package-path")
                .arg(std::path::absolute(packages)?);
        }
        Ok(command)
    }
}

impl TypstRunner for CommandTypst {
    fn prepare(&self, packages: Option<&Path>) -> Result<Vec<PathBuf>> {
        let namespace = match &self.package_path {
            Some(path) => path.clone(),
            None => crate::resource::package::default_namespace()?,
        };
        let mut roots = crate::resource::package::prepare_included_into(&namespace)?;
        if let Some(packages) = packages {
            roots.extend(crate::resource::package::prepare_packages_into(
                packages, &namespace,
            )?);
        }
        roots.sort();
        roots.dedup();
        Ok(roots)
    }

    fn identity(&self) -> Result<String> {
        let executable = self.executable()?;
        let output = Command::new(&executable)
            .arg("--version")
            .output()
            .with_context(|| {
                format!("Cannot run Typst executable {}", self.executable.display())
            })?;
        ensure!(
            output.status.success(),
            "Typst --version failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        // Environment settings can change fonts, imports, and rendered content.
        let environment = Self::environment()?;
        let package_path = self
            .package_path
            .as_ref()
            .map(std::path::absolute)
            .transpose()?;
        Ok(format!(
            "{}|{:?}|{:?}|{:?}",
            String::from_utf8_lossy(&output.stdout).trim(),
            executable,
            package_path,
            environment
        ))
    }

    fn export(&self, request: &ExportRequest<'_>) -> Result<Vec<PathBuf>> {
        crate::util::fs::create_all_parent_dir(request.output)?;
        let dependencies = request.output.with_extension("deps.json");
        let result = (|| {
            let output = self
                .command(request, &dependencies)?
                .output()
                .with_context(|| {
                    format!("Failed to export {} with Typst", request.input.display())
                })?;
            ensure!(
                output.status.success(),
                "Typst export failed for {}:\n{}",
                request.input.display(),
                String::from_utf8_lossy(&output.stderr)
            );
            read_dependencies(request.root, &dependencies, false)
        })();
        let _ = std::fs::remove_file(dependencies);
        result
    }

    fn pdf(&self, request: &PdfRequest<'_>) -> Result<Vec<PathBuf>> {
        crate::util::fs::create_all_parent_dir(request.output)?;
        let dependencies = request.output.with_extension("deps.json");
        let result = (|| {
            let mut command = self.compile_command(
                request.root,
                None,
                request.output,
                request.font_path,
                request.jobs,
                &dependencies,
                Format::Pdf,
            )?;
            let mut child = command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .context("Failed to start Typst PDF compilation")?;
            let mut stdin = child.stdin.take().context("Cannot open Typst stdin")?;
            // Drain stdout/stderr while writing even a large source catalog.
            let (output, written) = std::thread::scope(|scope| {
                let writer = scope.spawn(move || stdin.write_all(request.driver.as_bytes()));
                let output = child.wait_with_output();
                let written = writer.join();
                (output, written)
            });
            let output = output.context("Cannot wait for Typst PDF compilation")?;
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            ensure!(
                output.status.success(),
                "Typst PDF compilation failed:\n{diagnostic}{}",
                if diagnostic.contains("occurs multiple times") {
                    "\nNative Typst labels are document-global. Repeated articles must use unique native labels, or Typsite #anchor/#goto helpers for occurrence-local anchors."
                } else {
                    ""
                }
            );
            written.map_err(|_| anyhow::anyhow!("Typst stdin writer panicked"))??;
            ensure!(
                !diagnostic.contains("did not converge")
                    && !diagnostic.contains("did not stabilize"),
                "Typst PDF layout did not converge; refusing to publish an incomplete book:\n{diagnostic}"
            );
            if !diagnostic.trim().is_empty() {
                eprint!("{diagnostic}");
            }
            read_dependencies(request.root, &dependencies, true)
        })();
        let _ = std::fs::remove_file(dependencies);
        result
    }
}

fn read_dependencies(root: &Path, path: &Path, stdin: bool) -> Result<Vec<PathBuf>> {
    #[derive(Deserialize)]
    struct Dependencies {
        inputs: Vec<PathBuf>,
    }
    let dependencies: Dependencies = serde_json::from_slice(&std::fs::read(path)
        .context("Typst did not produce dependency metadata; use the latest stable Typst (tested with 0.15.1)")?)
        .context("Invalid Typst dependency metadata")?;
    let root = std::path::absolute(root)?;
    Ok(dependencies
        .inputs
        .into_iter()
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                root.join(path)
            }
        })
        .filter(|path| !stdin || path.file_name().is_none_or(|name| name != "<stdin>"))
        .collect())
}

fn absolute_environment_value(key: &str, value: OsString) -> Result<OsString> {
    if value.is_empty() {
        return Ok(value);
    }
    match key {
        "TYPST_FONT_PATHS" => Ok(std::env::join_paths(
            std::env::split_paths(&value)
                .map(std::path::absolute)
                .collect::<std::io::Result<Vec<_>>>()?,
        )?),
        "TYPST_PACKAGE_PATH" | "TYPST_PACKAGE_CACHE_PATH" => {
            Ok(std::path::absolute(value)?.into_os_string())
        }
        _ => Ok(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_executables_and_environment_paths_are_resolved_before_chdir() {
        let runner = CommandTypst::new(Path::new(".").join("typst with spaces"));
        assert_eq!(
            runner.executable().unwrap(),
            std::path::absolute("typst with spaces").unwrap()
        );
        let fonts = std::env::join_paths([Path::new("fonts one"), Path::new("fonts two")]).unwrap();
        let normalized = absolute_environment_value("TYPST_FONT_PATHS", fonts).unwrap();
        assert!(std::env::split_paths(&normalized).all(|path| path.is_absolute()));
        assert_eq!(
            absolute_environment_value("TYPST_PACKAGE_PATH", "relative packages".into()).unwrap(),
            std::path::absolute("relative packages")
                .unwrap()
                .into_os_string()
        );
    }

    #[test]
    fn arguments_are_portable_and_preserve_custom_roots_and_spaces() {
        let root = std::path::absolute("content with spaces").unwrap();
        let input = root.join("article.typ");
        let output = root.join("cache/article.html");
        let fonts = root.join("custom fonts");
        let dependencies = root.join("cache/article.deps.json");
        let request = ExportRequest {
            root: &root,
            input: &input,
            output: &output,
            font_path: &fonts,
            layout: Some("/lib/layout with spaces.typ"),
            jobs: NonZeroUsize::new(2).unwrap(),
        };
        let runner = CommandTypst::new("typst");
        let command = runner.command(&request, &dependencies).unwrap();
        assert_eq!(command.get_program(), "typst");
        let args: Vec<_> = command.get_args().collect();
        let root_arg = args.iter().position(|arg| *arg == "--root").unwrap();
        assert_eq!(args[root_arg + 1], root.as_os_str());
        // Windows absolute() normalizes separators; compare native path
        // components rather than the fixture's mixed-separator OsStr spelling.
        // Keep the mixed separator fixture to cover this exact CI regression.
        assert_eq!(Path::new(args[1]), input.as_path());
        assert_eq!(Path::new(args[2]), output.as_path());
        for path in [&fonts, &dependencies] {
            assert!(args.iter().any(|argument| Path::new(argument) == path));
        }
        assert!(args.iter().any(|arg| *arg == "--deps"));
        let jobs_arg = args.iter().position(|arg| *arg == "--jobs").unwrap();
        assert_eq!(args[jobs_arg + 1], "2");
        assert!(!args.iter().any(|arg| *arg == "powershell"));
        assert!(
            args.iter()
                .any(|arg| *arg == "typsite-layout=/lib/layout with spaces.typ")
        );
        assert!(args.iter().any(|arg| *arg == "typsite-source=/article"));
    }

    #[test]
    fn native_pdf_command_uses_stdin_and_an_explicit_format_not_html_features() {
        let root = std::path::absolute("source with spaces").unwrap();
        let command = CommandTypst::new("typst")
            .compile_command(
                &root,
                None,
                &root.join("book.pending.tmp"),
                &root.join("fonts"),
                NonZeroUsize::new(3).unwrap(),
                &root.join("book.deps.json"),
                Format::Pdf,
            )
            .unwrap();
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args[1], "-");
        assert!(args.windows(2).any(|pair| pair == ["--format", "pdf"]));
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--input", "typsite-pdf=true"])
        );
        assert!(!args.iter().any(|arg| *arg == "--features"));
        assert!(args.windows(2).any(|pair| pair == ["--jobs", "3"]));
    }
}
