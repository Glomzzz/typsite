use std::convert::Infallible;
use std::path::StripPrefixError;

#[derive(thiserror::Error, Debug)]
pub enum TypsiteError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Path conversion error: {0}")]
    PathConversion(#[from] StripPrefixError),

    #[error("Typst error: {0}")]
    Typst(String),

    #[error("HTML parsing error: {0}")]
    HtmlParse(#[from] Infallible),
}

pub fn log_err_or_ok<T, E: std::fmt::Debug>(result: anyhow::Result<T, E>) -> Option<T> {
    match result {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("[WARN] {e:?}");
            None
        }
    }
}
pub fn log_err<T, E: std::fmt::Debug>(result: anyhow::Result<T, E>) {
    match result {
        Ok(_) => {}
        Err(e) => eprintln!("[WARN] {e:?}"),
    }
}

pub fn padding_error(error: &anyhow::Error, padding: &str) -> String {
    error
        .to_string()
        .lines()
        .map(|it| format!("{padding}{it}"))
        .collect::<Vec<String>>()
        .join("\n")
}
