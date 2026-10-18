#!/usr/bin/env python3
"""Smoke-test a supplied/downloaded executable, never a cargo-run substitute.

Runs init -> compile -> edit -> compile -> failure/recovery -> clean rebuild
against real Typst in a temporary project with isolated package namespaces.
No process cwd/HOME or normal installed packages are changed.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import xml.etree.ElementTree as ET

from release_artifacts import verify, version


COUNTS = re.compile(r"(\d+) exported, (\d+) rendered, (\d+) written, (\d+) removed")


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def snapshot(root):
    return {path.relative_to(root).as_posix(): path.read_bytes()
            for path in root.rglob("*") if path.is_file()}


def smoke(binary, expected_version, typst, report_path=None, provenance=None):
    binary = binary.resolve(strict=True)
    typst = str(Path(typst).resolve()) if os.path.dirname(typst) else typst
    if os.name != "nt" and not os.access(binary, os.X_OK):
        binary.chmod(binary.stat().st_mode | 0o100)
    stages = []
    with tempfile.TemporaryDirectory(prefix="typsite binary smoke [fixture] ") as temporary:
        project = Path(temporary)
        env = os.environ.copy()
        # Never pass release credentials to the binary under test.
        for key in ("GH_TOKEN", "GITHUB_TOKEN", "GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN"):
            env.pop(key, None)
        env.update(
            TYPST_PACKAGE_PATH=str(project / "isolated packages"),
            TYPST_PACKAGE_CACHE_PATH=str(project / "isolated downloads"),
            TYPST_IGNORE_SYSTEM_FONTS="true",
            SOURCE_DATE_EPOCH="1772496000",
        )
        env.pop("TYPST_FONT_PATHS", None)
        env.pop("TYPST_IGNORE_EMBEDDED_FONTS", None)

        def run(arguments, success=True):
            result = subprocess.run([str(binary), *arguments], cwd=project, env=env,
                                    capture_output=True, text=True, encoding="utf-8", timeout=180)
            require((result.returncode == 0) == success,
                    f"{arguments}: exit {result.returncode}\n{result.stdout}\n{result.stderr}")
            return result.stdout

        def compile():
            output = run(["compile", "--jobs", "2", "--typst", typst])
            counts = COUNTS.search(output)
            require(counts is not None, f"Missing build report: {output}")
            return tuple(map(int, counts.groups()))

        require(run(["--version"]).strip() == f"typsite {expected_version}",
                "Executable version differs from the release metadata")
        typst_version = subprocess.check_output([typst, "--version"], env=env, text=True, encoding="utf-8").strip()
        run(["init"])
        options = project / ".typsite/options.toml"
        options.write_text(options.read_text(encoding="utf-8").replace(
            'base_url = ""', 'base_url = "https://example.org"'), encoding="utf-8")
        pages = [path for path in (project / "root").rglob("*.typ")
                 if not path.is_relative_to(project / "root/lib")]
        cold = compile()
        require(cold[0] == len(pages) and cold[1] == len(pages), f"Incomplete cold build: {cold}")
        for source in pages:
            page = project / "publish" / source.relative_to(project / "root").with_suffix(".html")
            html = page.read_text(encoding="utf-8")
            require("</html>" in html, f"Incomplete page: {page.name}")
            for unresolved in ("<schema", "<rewrite", "<embed", "<metacontent", "<metaoption"):
                require(unresolved not in html, f"Unresolved {unresolved} in {page.name}")
        feed = ET.parse(project / "publish/rss.xml").getroot()
        require(len(feed.findall("channel/item")) == 4, "Demo RSS selection is wrong")
        stages.append("init + complete demo + RSS")

        published = snapshot(project / "publish")
        watched = [path for directory in ("publish", ".cache")
                   for path in (project / directory).rglob("*") if path.is_file()]
        modified = {path: path.stat().st_mtime_ns for path in watched}
        require(compile() == (0, 0, 0, 0), "Warm build is not a true no-op")
        require(published == snapshot(project / "publish"), "No-op changed output bytes")
        require(all(path.stat().st_mtime_ns == stamp for path, stamp in modified.items()),
                "No-op rewrote cache/output files")
        stages.append("warm no-op")

        article = project / "root/en/typst.typ"
        with article.open("a", encoding="utf-8") as file:
            file.write("\nRelease binary incremental smoke marker.\n")
        require(compile()[0] == 1, "Article edit re-exported unrelated pages")
        for page in ("en/typst.html", "en/index.html"):
            require("Release binary incremental smoke marker." in
                    (project / "publish" / page).read_text(encoding="utf-8"),
                    f"Article/embedded edit did not reach {page}")
        stages.append("edit + selective export + embedded-page refresh")

        schema = project / ".typsite/schemas/page.html"
        original = schema.read_bytes()
        last_good = snapshot(project / "publish")
        schema.write_text("Invalid template without head/body", encoding="utf-8")
        run(["compile", "--jobs", "2", "--typst", typst], success=False)
        require(last_good == snapshot(project / "publish"), "Failed build changed last-good output")
        schema.write_bytes(original)
        require(compile()[0] == 0, "Template repair needlessly re-exported articles")
        stages.append("truthful failure + last-good output + repair")

        incremental = snapshot(project / "publish")
        run(["clean"])
        require(compile()[0] == len(pages), "Clean build did not export every article")
        require(incremental == snapshot(project / "publish"), "Incremental/clean output mismatch")
        stages.append("clean-build equivalence")
        require(not (project / "isolated downloads/preview").exists(),
                "Bundled demo downloaded preview packages")
    report = {
        "binary": binary.name,
        "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "version": expected_version,
        "typst": typst_version,
        "pages": len(pages),
        "passed": stages,
        "provenance": provenance,
    }
    if report_path:
        report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--typst", default=os.environ.get("TYPST", "typst"))
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--commit")
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    expected_version = version(Path(__file__).resolve().parent.parent)
    if bool(args.manifest) != bool(args.commit):
        parser.error("--manifest and --commit must be supplied together")
    provenance = verify(args.binary, args.manifest, args.commit, expected_version) if args.manifest else None
    smoke(args.binary, expected_version, args.typst, args.report, provenance)


if __name__ == "__main__":
    main()
