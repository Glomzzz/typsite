//! Typsite's build pipeline. The binary is a thin adapter around this library.

pub mod cli;
pub mod compile;
pub mod config;
pub mod ir;
pub mod pass;
pub mod resource;
pub mod util;

#[cfg(test)]
fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::{Mutex, OnceLock};

    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("test lock should not be poisoned")
}
