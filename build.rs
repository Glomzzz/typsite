// A deterministic engine identity avoids rereading the executable on every CLI
// invocation. Changes to code, bundled data, dependencies, compiler, target, or
// codegen settings invalidate the cache without relying on source timestamps.
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn hash_tree(root: &Path, path: &Path, hasher: &mut blake3::Hasher) {
    if path.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(path).unwrap().map(Result::unwrap).collect();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            hash_tree(root, &entry.path(), hasher);
        }
    } else {
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = fs::read(path).unwrap();
        hasher.update(&(relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
}

fn main() {
    let root = env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let root = Path::new(&root);
    let mut hasher = blake3::Hasher::new();
    for path in [
        "build.rs",
        "Cargo.toml",
        "Cargo.lock",
        "src",
        "resources",
        "packages",
    ] {
        println!("cargo:rerun-if-changed={path}");
        hash_tree(root, &root.join(path), &mut hasher);
    }
    for key in [
        "TARGET",
        "PROFILE",
        "OPT_LEVEL",
        "DEBUG",
        "CARGO_CFG_TARGET_FEATURE",
        "CARGO_CFG_FEATURE",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_PROFILE_RELEASE_LTO",
        "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
        "CARGO_PROFILE_RELEASE_PANIC",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
        hasher.update(key.as_bytes());
        hasher.update(env::var(key).unwrap_or_default().as_bytes());
        hasher.update(&[0]);
    }
    let compiler = Command::new(env::var_os("RUSTC").unwrap())
        .arg("-vV")
        .output()
        .expect("Cannot identify Rust compiler");
    assert!(compiler.status.success());
    hasher.update(&compiler.stdout);
    println!(
        "cargo:rustc-env=TYPSITE_ENGINE_ID={}",
        hasher.finalize().to_hex()
    );
}
