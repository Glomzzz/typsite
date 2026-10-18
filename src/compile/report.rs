use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Discover,
    Export,
    Parse,
    Resolve,
    Render,
    Publish,
    Cache,
}

#[derive(Debug)]
pub struct Diagnostic {
    pub stage: Stage,
    pub path: Option<PathBuf>,
    pub message: String,
}

/// A successful computation is not necessarily a change to published output.
#[derive(Debug, Default)]
pub struct BuildReport {
    pub exported: Vec<PathBuf>,
    pub rendered: usize,
    pub written: Vec<PathBuf>,
    pub removed: Vec<PathBuf>,
    pub diagnostics: Vec<Diagnostic>,
}

impl BuildReport {
    pub fn is_success(&self) -> bool {
        self.diagnostics.is_empty()
    }
    pub fn changed(&self) -> bool {
        !self.written.is_empty() || !self.removed.is_empty()
    }
    pub(crate) fn error(
        &mut self,
        stage: Stage,
        path: Option<PathBuf>,
        error: impl std::fmt::Display,
    ) {
        self.diagnostics.push(Diagnostic {
            stage,
            path,
            message: error.to_string(),
        });
    }
}
