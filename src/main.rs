use crate::cli::cli;

pub(crate) mod cli;
pub(crate) mod compile;
pub(crate) mod config;
pub(crate) mod ir;
pub(crate) mod pass;
pub(crate) mod resource;
pub(crate) mod util;

#[allow(dead_code)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cli().await
}
