//! The only boundary that invokes the Typst executable.

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct ExportRequest<'a> {
    pub root: &'a Path,
    pub input: &'a Path,
    pub output: &'a Path,
    pub font_path: &'a Path,
}

/// Implementations must write HTML to `output` and return the files actually read
/// by Typst. This small seam lets build tests run without a shell or package installs.
pub trait TypstRunner: Send + Sync {
    fn identity(&self) -> Result<String>;
    fn export(&self, request: &ExportRequest<'_>) -> Result<Vec<PathBuf>>;
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

    fn command(&self, request: &ExportRequest<'_>, dependencies: &Path) -> Result<Command> {
        let mut command = Command::new(&self.executable);
        command
            .current_dir(std::path::absolute(request.root)?)
            .arg("compile")
            .arg(std::path::absolute(request.input)?)
            .arg(std::path::absolute(request.output)?)
            .arg("--root")
            .arg(std::path::absolute(request.root)?)
            .args([
                "--format",
                "html",
                "--features",
                "html",
                "--input",
                "html-frames=true",
            ])
            .arg("--font-path")
            .arg(std::path::absolute(request.font_path)?)
            .arg("--deps")
            .arg(std::path::absolute(dependencies)?)
            .args(["--deps-format", "json"]);
        if let Some(packages) = &self.package_path {
            command
                .arg("--package-path")
                .arg(std::path::absolute(packages)?);
        }
        Ok(command)
    }
}

impl TypstRunner for CommandTypst {
    fn identity(&self) -> Result<String> {
        let output = Command::new(&self.executable)
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
        let environment: Vec<_> = [
            "TYPST_FONT_PATHS",
            "TYPST_IGNORE_SYSTEM_FONTS",
            "TYPST_IGNORE_EMBEDDED_FONTS",
            "TYPST_PACKAGE_PATH",
            "TYPST_PACKAGE_CACHE_PATH",
            "SOURCE_DATE_EPOCH",
        ]
        .into_iter()
        .map(|key| (key, std::env::var_os(key)))
        .collect();
        Ok(format!(
            "{}|{:?}|{:?}|{:?}",
            String::from_utf8_lossy(&output.stdout).trim(),
            self.executable,
            self.package_path,
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
            #[derive(Deserialize)]
            struct Dependencies {
                inputs: Vec<PathBuf>,
            }
            let dependencies: Dependencies = serde_json::from_slice(&std::fs::read(&dependencies)
                .context("Typst did not produce dependency metadata; use the latest stable Typst (tested with 0.15.1)")?)
                .context("Invalid Typst dependency metadata")?;
            let root = std::path::absolute(request.root)?;
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
                .collect())
        })();
        let _ = std::fs::remove_file(dependencies);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        };
        let runner = CommandTypst::new("typst");
        let command = runner.command(&request, &dependencies).unwrap();
        assert_eq!(command.get_program(), "typst");
        let args: Vec<_> = command.get_args().collect();
        let root_arg = args.iter().position(|arg| *arg == "--root").unwrap();
        assert_eq!(args[root_arg + 1], root.as_os_str());
        assert!(args.contains(&input.as_os_str()));
        assert!(args.contains(&output.as_os_str()));
        assert!(args.contains(&fonts.as_os_str()));
        assert!(args.iter().any(|arg| *arg == "--deps"));
        assert!(!args.iter().any(|arg| *arg == "powershell"));
    }
}
