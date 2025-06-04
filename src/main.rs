#![feature(str_as_str)]
#![feature(let_chains)]
#![feature(path_add_extension)]
#![feature(future_join)]

use crate::cli::cli;

pub(crate) mod ir;
pub(crate) mod cli;
pub(crate) mod compile;
pub(crate) mod config;
pub(crate) mod pass;
pub(crate) mod util;

#[allow(dead_code)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cli().await
}
