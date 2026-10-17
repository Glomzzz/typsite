//! Minimal, structurally valid configuration shared by unit tests.

use std::fs;
use std::path::Path;

pub(crate) fn write_config(root: &Path) {
    write_config_with_options(root, include_str!("../resources/.typsite/options.toml"));
}

pub(crate) fn write_config_with_options(root: &Path, options: &str) {
    for dir in [
        "components/footer",
        "rewrite",
        "schemas",
        "syntaxes",
        "themes",
    ] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    fs::write(root.join("options.toml"), options).unwrap();
    fs::write(
        root.join("components/section.html"),
        include_str!("../resources/.typsite/components/section.html"),
    )
    .unwrap();
    for (path, body) in [
        ("components/heading-numbering.html", "{numbering}"),
        (
            "components/footer.html",
            "<footer>{references}{backlinks}</footer>",
        ),
        ("components/footer/backlinks.html", "{backlinks}"),
        ("components/footer/references.html", "{references}"),
        ("components/anchor_def.html", ""),
        ("components/anchor_def_svg.html", ""),
        ("components/anchor_goto.html", ""),
        ("components/anchor_goto_svg.html", ""),
        ("components/sidebar.html", "{children}"),
        ("components/sidebar_each.html", "{title}"),
        ("components/embed.html", "{content}"),
        ("components/embed_title.html", "{title}"),
        ("schemas/main.html", "<content></content>"),
        ("schemas/reference.html", "<content></content>"),
    ] {
        fs::write(root.join(path), format!("<head></head><body>{body}</body>")).unwrap();
    }
}
