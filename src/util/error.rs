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

pub fn padding_error(error: &anyhow::Error, padding: &str) -> String {
    error
        .to_string()
        .lines()
        .map(|it| format!("{padding}{it}"))
        .collect::<Vec<String>>()
        .join("\n")
}
