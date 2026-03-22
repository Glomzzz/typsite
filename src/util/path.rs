use crate::util::error::TypsiteError;
use anyhow::*;
use std::path::{Component, Path, PathBuf};

pub fn file_stem(path: &Path) -> Option<&str> {
    path.file_stem()?.to_str()
}

pub fn format_path_ref(path: &Path) -> &Path {
    path.strip_prefix("./").unwrap_or(path)
}
pub fn format_path(path: PathBuf) -> PathBuf {
    path.strip_prefix("./")
        .map(|it| it.to_path_buf())
        .unwrap_or(path)
}

pub fn verify_if_relative_path<P: AsRef<Path>>(cwd: &Path, path: P) -> Result<PathBuf> {
    let path = path.as_ref();
    if path.starts_with("/") {
        
        relative_path(cwd, path)
    } else {
        Ok(path.to_path_buf())
    }
    .map(format_path)
    .context("Absolute paths are not supported. Please use a relative path from the current working directory.")
}

pub fn relative_path(base: &Path, path: &Path) -> Result<PathBuf> {
    path.strip_prefix(base)
        .map(|p| p.to_path_buf())
        .map_err(TypsiteError::PathConversion)
        .with_context(|| format!("Failed to convert path {path:?} to relative path of {base:?}"))
}

pub fn dir_name(path: &Path) -> Option<&str> {
    if path.is_dir() {
        path.file_name()?.to_str()
    } else {
        path.parent()?.file_name()?.to_str()
    }
}

pub fn file_ext(path: &Path) -> Option<String> {
    Some(path.extension()?.to_string_lossy().to_string())
}

pub fn resolve_path(root_path: &Path, current_path: &Path, path: &str) -> Result<PathBuf> {
    let (base_path, trimmed_slug) = if path.starts_with('/') {
        (root_path, path.trim_start_matches('/'))
    } else {
        (current_path, path)
    };
    let combined = base_path.join(trimmed_slug);
    let normalized = normalize_path(&combined);
    if normalized.starts_with(root_path) {
        Ok(normalized)
    } else {
        Err(anyhow!(
            "Path '{}' escapes root directory '{}'",
            normalized.display(),
            root_path.display()
        ))
    }
}
fn normalize_path(path: &Path) -> PathBuf {
    let mut stack = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir => {
                stack.clear();
                stack.push(Component::RootDir);
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if let Some(Component::Normal(_)) = stack.last() {
                    stack.pop();
                } else {
                    stack.push(Component::ParentDir);
                }
            }
            Component::Normal(_) => stack.push(component),
            Component::Prefix(prefix) => stack.push(Component::Prefix(prefix)),
        }
    }
    stack.iter().fold(PathBuf::new(), |mut acc, c| {
        acc.push(c.as_os_str());
        acc
    })
}
#[test]
fn test_resolve_path() {
    let root = "/app/root";
    let current = "/app/root/current/dir";
    // Absolute path test
    assert_eq!(
        resolve_path(root.as_ref(), current.as_ref(), "/foo/bar").unwrap(),
        PathBuf::from("/app/root/foo/bar")
    );
    // Relative path test
    assert_eq!(
        resolve_path(root.as_ref(), current.as_ref(), "../../config").unwrap(),
        PathBuf::from("/app/root/config")
    );
    // Path traversal test
    assert!(resolve_path(root.as_ref(), current.as_ref(), "/../../etc/passwd").is_err());
}
