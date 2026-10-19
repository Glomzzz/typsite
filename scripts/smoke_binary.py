#!/usr/bin/env python3
"""Smoke-test a supplied/downloaded executable, never a cargo-run substitute.

Runs HTML and native PDF builds, edits, no-ops, failure/recovery, and clean
rebuilds against real Typst in a temporary project with isolated namespaces.
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


def file_stamps(paths):
    return {path: path.stat().st_mtime_ns for root in paths
            for path in ([root] if root.is_file() else root.rglob("*")) if path.is_file()}


def read_pdf(path):
    data = path.read_bytes()
    require(data.startswith(b"%PDF-") and len(data) > 1024,
            f"Missing or incomplete native PDF: {path}")
    return data


def smoke_pdf(project, run, compile_html, stages):
    """Use the supplied executable for every build; Typst assertions in its
    template verify native structure without platform-specific PDF utilities.
    This fixture needs neither system CJK fonts nor remote font/package access.
    """
    library = project / "root/lib"
    (library / "release-front.typ").write_text(
        '#metadata("release-front")\n#include "front-cover.typ"\n', encoding="utf-8")
    (library / "release-back.typ").write_text(
        '#metadata("release-back")\n#include "back-cover.typ"\n', encoding="utf-8")
    appendix = library / "release-appendix.typ"
    appendix_source = ('#import "/lib/lib.typ": *\n#show: schema.with("page")\n'
                       '#title[Release appendix]\nRelease PDF appendix version one.\n')
    appendix.write_text(appendix_source, encoding="utf-8")
    layout = library / "release-layout.typ"
    layout_source = '''#let page-header(info) = {
  metadata((release-header: info.page, target: info.target))
  [Release shared header version one: #info.title]
}
#let page-footer(info) = {
  metadata((release-footer: info.page, target: info.target))
  [Release footer #info.page]
}
'''
    layout.write_text(layout_source, encoding="utf-8")
    (library / "release-pdf.typ").write_text('''#import "pdf.typ" as print-style
#let book-style(body) = {
  set page(header: context [Release template header
    #metadata((release-native-header: counter(page).get().first()))])
  print-style.book-style([
    #body
    #context {
      let facts = query(metadata)
      let records(name) = facts.filter(it => type(it.value) == dictionary and
        it.value.at("typsite-pdf", default: none) == name)
      let articles = records("article")
      assert(articles.map(it => it.value.scope.source) == (
        "/index", "/typst", "/article", "/migrate-to-016", "/migrate-to-017",
        "/migrate-to-018", "/example", "/typst", "/lib/release-appendix"),
        message: "Release PDF forest and ordered appendices")
      let front = facts.find(it => it.value == "release-front")
      let back = facts.find(it => it.value == "release-back")
      assert(front.location().page() == 1, message: "Front cover must be first")
      assert(back.location().page() == counter(page).final().first() + 1,
        message: "Back cover must be last with numbering reset after the front")
      assert(articles.first().location().page() > 1 and
        articles.last().location().page() < back.location().page())
      let pages = range(2, back.location().page())
      for key in ("release-header", "release-footer", "release-native-header") {
        let furniture = facts.filter(it => type(it.value) == dictionary and key in it.value)
        assert(furniture.map(it => it.location().page()) == pages,
          message: "Exactly one " + key + " on every non-cover page")
        assert(furniture.map(it => it.value.at(key)) == range(1, pages.len() + 1),
          message: "Contextual page furniture receives the right page number")
      }
      for item in query(heading).filter(it => it.numbering != none) {
        assert(not counter(heading).at(item.location()).contains(0),
          message: "Release PDF heading numbers must have no zero components")
      }
      assert(query(image).len() >= 2, message: "Original cover and article images")
      assert(query(math.equation).len() > 0, message: "Equations remain native")
      assert(query(footnote).len() > 0, message: "Demo notes reach the book")
    }
  ])
}
''', encoding="utf-8")
    options = project / ".typsite/options.toml"
    configuration = options.read_text(encoding="utf-8")
    require('module = ""' in configuration, "Initialized project has no optional shared layout setting")
    options.write_text(configuration.replace('module = ""', 'module = "lib/release-layout.typ"'),
                       encoding="utf-8")
    compile_html()
    for page in (project / "publish").rglob("*.html"):
        if page.is_relative_to(project / "publish/assets"):
            continue
        html = page.read_text(encoding="utf-8")
        require(html.count('class="page-header"') == 1 and
                html.count('class="page-footer"') == 1,
                f"Shared layout missing or repeated by embeds in {page.name}")
    published = snapshot(project / "publish")
    published_stamps = file_stamps([project / "publish"])
    output = project / "book/site.pdf"
    arguments = ["pdf", "--entry", "index.typ", "--output", "book/site.pdf",
                 "--template", "lib/release-pdf.typ", "--front-cover", "lib/release-front.typ",
                 "--back-cover", "lib/release-back.typ", "--title", "Release smoke book",
                 "--font-profile", "none", "--offline", "--jobs", "2"]
    for source in ("migrate-to-016.typ", "migrate-to-017.typ", "migrate-to-018.typ",
                   "example.typ", "lib/release-appendix.typ"):
        arguments += ["--appendix", source]

    def pdf(success=True, diagnostic=None):
        return run(arguments, success=success, diagnostic=diagnostic)

    def unchanged_html():
        require(snapshot(project / "publish") == published and
                file_stamps([project / "publish"]) == published_stamps,
                "PDF build modified HTML artifacts or ownership")

    require("(compiled)" in pdf(), "First native PDF build was not compiled")
    read_pdf(output)
    unchanged_html()
    stages.append("PDF complete demo + covers + shared layout + HTML isolation")

    modified = file_stamps([output, project / ".cache"])
    cached = snapshot(project / ".cache")
    previous = read_pdf(output)
    require("(cached)" in pdf(), "Warm PDF build was not cached")
    require(read_pdf(output) == previous and snapshot(project / ".cache") == cached,
            "PDF no-op changed cache/output bytes")
    require(file_stamps([output, project / ".cache"]) == modified,
            "PDF no-op rewrote cache/output files")
    unchanged_html()
    stages.append("PDF warm no-op")

    appendix.write_text(appendix_source.replace("version one", "version two"), encoding="utf-8")
    require("(compiled)" in pdf() and read_pdf(output) != previous,
            "Print-only appendix edit did not rebuild the PDF")
    previous = read_pdf(output)
    layout.write_text(layout_source.replace("version one", "version two"), encoding="utf-8")
    require("(compiled)" in pdf() and read_pdf(output) != previous,
            "Shared layout edit did not rebuild the PDF")
    unchanged_html()
    compile_html()
    published = snapshot(project / "publish")
    published_stamps = file_stamps([project / "publish"])
    stages.append("PDF incremental appendix + layout refresh")

    previous = read_pdf(output)
    modified = file_stamps([output])
    valid = appendix.read_bytes()
    appendix.write_text(appendix_source + '#context [\n== Unsafe contextual heading\n]\n', encoding="utf-8")
    pdf(success=False, diagnostic="cannot be normalized safely")
    require(read_pdf(output) == previous and file_stamps([output]) == modified,
            "Failed native PDF build changed the last-good book")
    unchanged_html()
    appendix.write_bytes(valid)
    require("(cached)" in pdf() and read_pdf(output) == previous,
            "PDF repair did not recover the verified last-good book")
    unchanged_html()
    stages.append("PDF truthful failure + last-good output + repair")

    run(["clean"])
    require(read_pdf(output) == previous, "HTML cleanup removed the standalone book")
    compile_html()
    require(snapshot(project / "publish") == published, "Shared-layout HTML clean build differs")
    require("(compiled)" in pdf() and read_pdf(output) == previous,
            "Native PDF incremental/clean output mismatch")
    stages.append("PDF clean-build equivalence")
    return {"sha256": hashlib.sha256(previous).hexdigest(), "size": len(previous)}


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

        def run(arguments, success=True, diagnostic=None):
            if arguments and arguments[0] == "pdf":
                arguments = [*arguments, "--typst", typst]
            result = subprocess.run([str(binary), *arguments], cwd=project, env=env,
                                    capture_output=True, text=True, encoding="utf-8", timeout=180)
            require((result.returncode == 0) == success,
                    f"{arguments}: exit {result.returncode}\n{result.stdout}\n{result.stderr}")
            if diagnostic is not None:
                require(diagnostic in result.stderr, f"Expected {diagnostic!r}: {result.stderr}")
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
        pdf = smoke_pdf(project, run, compile, stages)
        require(not (project / "isolated downloads/preview").exists(),
                "Bundled demo downloaded preview packages")
    report = {
        "binary": binary.name,
        "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "version": expected_version,
        "typst": typst_version,
        "pages": len(pages),
        "pdf": pdf,
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
