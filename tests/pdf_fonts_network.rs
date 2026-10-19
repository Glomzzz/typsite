//! Optional end-to-end network test; not part of offline/real-Typst CI jobs.
//! Run explicitly: cargo test --test pdf_fonts_network -- --ignored
mod common;

use common::Project;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[test]
#[ignore = "requires Typst and HTTPS access to the pinned official Noto font assets"]
fn remote_profile_is_pinned_cached_offline_and_preserves_last_good_output() {
    let project = Project::named("typsite remote font [fixture]");
    typsite::util::fs::copy_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/root/lib"),
        project.input.join("lib"),
    )
    .unwrap();
    project.source("index.typ", "#import \"/lib/lib.typ\": *\n#show: schema.with(\"page\")\n#title[远程字体]\n中文正文和 *较轻的强调*。\n= 中文标题\n#LaTeX 和 Typst。\n$ x^2 $");
    let output = project.root.join("book/site.pdf");
    let run = |offline: bool| -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_typsite"));
        command
            .args(["pdf", "--input"])
            .arg(&project.input)
            .arg("--config")
            .arg(&project.config)
            .arg("--cache")
            .arg(&project.cache)
            .arg("--output")
            .arg(&output)
            .args([
                "--template",
                "lib/pdf.typ",
                "--font-profile",
                "noto-serif-sc",
            ])
            .arg("--typst")
            .arg(std::env::var_os("TYPST").unwrap_or_else(|| "typst".into()))
            .env("TYPST_PACKAGE_PATH", project.root.join("packages"))
            .env(
                "TYPST_PACKAGE_CACHE_PATH",
                project.root.join("package-cache"),
            )
            .env("TYPST_IGNORE_SYSTEM_FONTS", "true")
            .env("SOURCE_DATE_EPOCH", "315532800")
            .env_remove("TYPST_FONT_PATHS")
            .env_remove("TYPST_IGNORE_EMBEDDED_FONTS");
        if offline {
            command.arg("--offline");
        }
        command.output().unwrap()
    };
    let success = |result: Output| -> Output {
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!String::from_utf8_lossy(&result.stderr).contains("unknown font family"));
        result
    };
    let first = success(run(false));
    assert!(String::from_utf8_lossy(&first.stderr).contains("Downloading PDF font resource"));
    let good = fs::read(&output).unwrap();
    let modified = fs::metadata(&output).unwrap().modified().unwrap();
    let warm = success(run(true));
    assert!(String::from_utf8_lossy(&warm.stdout).contains("(cached)"));
    assert!(!String::from_utf8_lossy(&warm.stderr).contains("Downloading"));
    assert_eq!(fs::metadata(&output).unwrap().modified().unwrap(), modified);
    let fonts = fs::read_dir(project.cache.join("pdf/fonts"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(fonts.join("OFL.txt").is_file());
    fs::write(fonts.join("NotoSerifSC-Regular.otf"), "corrupt").unwrap();
    let failed = run(true);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("missing or corrupt"));
    assert_eq!(fs::read(&output).unwrap(), good);
    success(run(false));
    assert_eq!(fs::read(&output).unwrap(), good);
    // Forget compilation manifests/artifacts but keep the separately verified
    // font cache, then check deterministic clean/incremental equivalence.
    for entry in fs::read_dir(project.cache.join("pdf")).unwrap() {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_some_and(|extension| extension == "pdf" || extension == "json")
        {
            fs::remove_file(path).unwrap();
        }
    }
    success(run(true));
    assert_eq!(fs::read(&output).unwrap(), good);
    assert!(
        !project
            .config
            .join("assets/fonts/NotoSerifSC-Regular.otf")
            .exists()
    );
}
