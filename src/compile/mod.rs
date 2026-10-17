use crate::compile::options::CompileOptions;
use anyhow::*;
use options::ProjOptions;
use std::result::Result::Ok;
use std::sync::{OnceLock, RwLock, RwLockReadGuard};

pub mod compiler;
pub mod options;
pub mod watch;

pub mod error;
pub mod registry;

macro_rules! global_config {
    ($static_name:ident, $type:ty, $init_fn:ident, $accessor:ident) => {
        pub static $static_name: OnceLock<RwLock<$type>> = OnceLock::new();

        pub fn $init_fn(config: $type) -> Result<()> {
            if let Some(lock) = $static_name.get() {
                let mut guard = lock.write().map_err(|_| {
                    anyhow!(
                        "Failed to acquire write lock for {}",
                        stringify!($static_name)
                    )
                })?;
                *guard = config;
            } else {
                $static_name
                    .set(RwLock::new(config))
                    .map_err(|_| anyhow!("Failed to initialize {}", stringify!($static_name)))?;
            }
            Ok(())
        }

        /// 获取全局配置的只读访问
        pub fn $accessor() -> Result<RwLockReadGuard<'static, $type>> {
            let lock = $static_name
                .get()
                .ok_or_else(|| anyhow!("{} not initialized", stringify!($static_name)))?;
            lock.read().map_err(|_| {
                anyhow!(
                    "Failed to acquire read lock for {}",
                    stringify!($static_name)
                )
            })
        }
    };
}

global_config!(
    COMPILE_OPTIONS,
    CompileOptions,
    init_compile_options,
    compile_options
);

global_config!(OPTIONS, ProjOptions, init_proj_options, proj_options);
