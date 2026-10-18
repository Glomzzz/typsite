// Standalone, dependency-free executable compiled into an isolated test project.
// This validates the CLI/process boundary, not real Typst compatibility.
use std::{env, fs, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args == ["--version"] {
        println!("typst 0.15.1 (test stub)");
        return Ok(());
    }
    assert_eq!(args[0], "compile");
    let option = |name: &str| -> &str { &args[args.iter().position(|arg| arg == name).unwrap() + 1] };
    let input = Path::new(&args[1]);
    let output = Path::new(&args[2]);
    let root = Path::new(option("--root"));
    assert!(root.is_absolute());
    assert!(input.starts_with(root));
    assert_eq!(option("--deps-format"), "json");
    assert_eq!(option("--format"), "html");
    let jobs: usize = option("--jobs").parse()?;
    assert!(jobs >= 1);
    if let Some(log) = env::var_os("TYPSITE_TEST_JOB_LOG") {
        fs::write(log, jobs.to_string())?;
    }
    assert_eq!(fs::canonicalize(env::current_dir()?)?, fs::canonicalize(root)?);
    fs::copy(input, output)?;
    fs::write(option("--deps"), format!("{{\"inputs\":[{:?}]}}", args[1]))?;
    if let Some(log) = env::var_os("TYPSITE_TEST_EXPORT_LOG") {
        use std::io::Write;
        writeln!(fs::OpenOptions::new().create(true).append(true).open(log)?, "{}", input.display())?;
    }
    Ok(())
}
