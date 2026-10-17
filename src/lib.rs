//! Typsite's build pipeline. The binary is a thin adapter around this library.

pub mod cli;
pub mod compile;
pub mod config;
pub mod ir;
pub mod pass;
pub mod resource;
pub mod util;
