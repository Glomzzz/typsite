//! The only boundary that invokes the Typst executable.

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::ffi::OsString;
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
    /// Prepare package resources in the same environment used by `export`.
    /// Test runners need no filesystem installation side effects.
    fn prepare(&self, _packages: Option<&Path>) -> Result<()> {
        Ok(())
    }
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
        let mut command = Command::new(self.executable()?);
        for (key, value) in Self::environment()? {
            if let Some(value) = value {
                command.env(key, value);
            } else {
                command.env_remove(key);
            }
        }
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
    fn prepare(&self, packages: Option<&Path>) -> Result<()> {
        let namespace = match &self.package_path {
            Some(path) => path.clone(),
            None => crate::resource::package::default_namespace()?,
        };
        crate::resource::package::install_included_into(&namespace)?;
        if let Some(packages) = packages {
            crate::resource::package::install_packages_into(packages, &namespace)?;
        }
        Ok(())
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
