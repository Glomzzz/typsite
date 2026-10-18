#!/usr/bin/env python3
"""Benchmark real-CLI builds without touching normal packages or the checkout.

Example (build/copy each revision's release binary first):
  python3 scripts/benchmark-build.py --binary /tmp/typsite-before --label before
  python3 scripts/benchmark-build.py --binary target/release/typsite --label after
  python3 scripts/benchmark-build.py --binary target/release/typsite --jobs 1

Timing is wall-clock, not a portable performance assertion. Each run uses the
same bundled demo plus generated independent articles, a cold Typsite cache,
isolated package paths, bundled/embedded fonts, and a fixed timestamp. Output
hashes are recorded so correctness can be compared alongside speed.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import statistics
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--label", default="current")
    parser.add_argument("--resources", type=Path, help="Optional resource tree shared by both versions")
    parser.add_argument("--independent-only", action="store_true", help="Use generated articles only, retaining the chosen library/config")
    parser.add_argument("--jobs", type=int)
    parser.add_argument("--pages", type=int, default=64)
    parser.add_argument("--note", default="", help="Record timing conditions, e.g. paused preview")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.runs < 1 or args.pages < 1 or (args.jobs is not None and args.jobs < 1):
        parser.error("runs, pages, and jobs must be positive")
    binary = args.binary.resolve()
    repository = Path(__file__).resolve().parent.parent
    results = []
    with tempfile.TemporaryDirectory(prefix="typsite-benchmark-") as temporary:
        root = Path(temporary)
        for iteration in range(args.runs):
            project = root / f"project-{iteration}"
            shutil.copytree(args.resources or repository / "resources", project)
            if args.independent_only:
                for path in (project / "root").rglob("*.typ"):
                    if not path.is_relative_to(project / "root/lib"):
                        path.unlink()
            for index in range(args.pages):
                source = (
                    '#import "/lib/lib.typ": *\n#show: schema.with("page")\n'
                    f'#title[Benchmark article {index:04}]\n#date[2026-03-02 18:25]\n'
                    '= First section\nA paragraph with *emphasis*, a footnote'
                    '#footnote[Benchmark note], and $x^2 + y^2 = z^2$.\n'
                    '== Code\n```rust\nfn main() { println!("hello"); }\n```\n'
                    + ("A reproducible paragraph with _some content_.\n\n" * 20)
                )
                path = project / f"root/benchmark-{index:04}.typ"
                path.write_text(source, encoding="utf-8")
            env = os.environ.copy()
            # 0.1.7 installs into HOME/.cache and ignores explicit package paths.
            # Child-only HOME also isolates that unchanged legacy executable.
            home = project / "isolated-home"
            home.mkdir()
            env.update(
                HOME=str(home),
                TYPST_PACKAGE_PATH=str(home / ".cache/typst/packages"),
                TYPST_PACKAGE_CACHE_PATH=str(project / "isolated-downloads"),
                TYPST_IGNORE_SYSTEM_FONTS="true",
                SOURCE_DATE_EPOCH="1772496000",
            )
            env.pop("TYPST_FONT_PATHS", None)
            env.pop("TYPST_IGNORE_EMBEDDED_FONTS", None)
            command = [str(binary), "compile"]
            if args.jobs is not None:
                command += ["--jobs", str(args.jobs)]
            for scenario in ["cold", "noop", "template", "one-article"]:
                if scenario == "template":
                    schema = project / ".typsite/schemas/page.html"
                    schema.write_text(schema.read_text(encoding="utf-8").replace(
                        '<article>', '<article data-benchmark="template-edit">'
                    ), encoding="utf-8")
                if scenario == "one-article":
                    article = project / "root/benchmark-0000.typ"
                    with article.open("a", encoding="utf-8") as file:
                        file.write("\nAn incremental article edit.\n")
                start = time.perf_counter()
                output = subprocess.run(
                    command, cwd=project, env=env, capture_output=True, text=True, check=True
                )
                elapsed = time.perf_counter() - start
                counts = re.search(
                    r"(\d+) exported, (\d+) rendered, (\d+) written, (\d+) removed", output.stdout
                )
                if not counts:
                    assert "Compiling done." in output.stdout and not output.stderr.strip(), (output.stdout, output.stderr)
                articles = sum(1 for path in (project / "root").rglob("*.typ") if not path.is_relative_to(project / "root/lib"))
                assert sum(1 for _ in (project / "publish").rglob("*.html")) == articles, (output.stdout, output.stderr)
                if scenario == "template":
                    assert all(b'data-benchmark="template-edit"' in path.read_bytes() for path in (project / "publish").rglob("*.html"))
                if scenario == "one-article":
                    assert b"An incremental article edit." in (project / "publish/benchmark-0000.html").read_bytes()
                digest = hashlib.sha256()
                for path in sorted((project / "publish").rglob("*")):
                    if path.is_file() and not path.name.startswith(".typsite"):
                        digest.update(path.relative_to(project / "publish").as_posix().encode())
                        digest.update(b"\0")
                        digest.update(path.read_bytes())
                result = dict(
                    run=iteration, scenario=scenario, seconds=elapsed,
                    counts=list(map(int, counts.groups())) if counts else None, output_hash=digest.hexdigest(),
                )
                results.append(result)
                print(f"{args.label} run={iteration + 1} {scenario}: {elapsed:.3f}s {counts.group(0) if counts else 'legacy output verified'}", flush=True)
    summary = {
        scenario: statistics.median(result["seconds"] for result in results if result["scenario"] == scenario)
        for scenario in ["cold", "noop", "template", "one-article"]
    }
    report = dict(
        label=args.label, binary=str(binary), jobs=args.jobs, generated_pages=args.pages,
        resources=str(args.resources) if args.resources else None, independent_only=args.independent_only,
        note=args.note,
        binary_hash=hashlib.sha256(binary.read_bytes()).hexdigest(),
        runs=args.runs, typst=subprocess.check_output(["typst", "--version"], text=True).strip(),
        cpu_count=os.cpu_count(), rayon_num_threads=os.environ.get("RAYON_NUM_THREADS"),
        median_seconds=summary, results=results,
    )
    print(json.dumps(report, indent=2))
    if args.output:
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
