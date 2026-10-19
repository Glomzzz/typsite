#!/usr/bin/env python3
"""Stage/download/validate draft releases; intentionally cannot publish one.

Only push-to-main workflows call this script with a write token. Every mutation
checks draft status and exact target SHA. Existing public releases are immutable
to this workflow, including reruns. No secrets are written into provenance.
"""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
from urllib.parse import quote

from release_artifacts import TARGETS, checked_commit, manifest, verify, version


REQUIRED_STAGES = {
    "init + complete demo + RSS",
    "warm no-op",
    "edit + selective export + embedded-page refresh",
    "truthful failure + last-good output + repair",
    "clean-build equivalence",
    "PDF complete demo + covers + shared layout + HTML isolation",
    "PDF warm no-op",
    "PDF incremental appendix + layout refresh",
    "PDF truthful failure + last-good output + repair",
    "PDF clean-build equivalence",
}


def gh(*arguments):
    return subprocess.check_output(["gh", *map(str, arguments)], text=True, encoding="utf-8")


def find_release(repository, tag):
    pages = json.loads(gh("api", "--paginate", "--slurp", f"repos/{repository}/releases?per_page=100"))
    matches = [release for page in pages for release in page if release["tag_name"] == tag]
    if len(matches) > 1:
        raise ValueError("Ambiguous release tag")
    return matches[0] if matches else None


def require_draft(release, commit):
    if release is None or not release["draft"] or release["target_commitish"] != commit:
        raise ValueError("Refusing to modify/download a non-draft release or a different revision")


def require_matching_tag(repository, tag, commit):
    refs = json.loads(gh("api", f"repos/{repository}/git/matching-refs/tags/{quote(tag, safe='')}"))
    exact = [ref for ref in refs if ref["ref"] == f"refs/tags/{tag}"]
    if not exact:
        # GitHub can defer creating a draft release's tag until publication.
        return
    if len(exact) != 1:
        raise ValueError("Ambiguous release tag reference")
    target = exact[0]["object"]
    seen = set()
    while target["type"] == "tag":
        if target["sha"] in seen or len(seen) >= 8:
            raise ValueError("Invalid annotated release tag chain")
        seen.add(target["sha"])
        target = json.loads(gh("api", f"repos/{repository}/git/tags/{target['sha']}"))["object"]
    if target["type"] != "commit" or target["sha"] != commit:
        raise ValueError("Release tag points at a different source revision")


def checked_draft(repository, tag, commit):
    release = find_release(repository, tag)
    require_draft(release, commit)
    require_matching_tag(repository, tag, commit)
    return release


def notes(repository_root, repository, commit, ready=False, assets=()):
    body = (repository_root / f"docs/releases/{version(repository_root)}.md").read_text(encoding="utf-8")
    body = body.replace("/blob/main/", f"/blob/{commit}/")
    status = "VALIDATED DRAFT — manual publication required" if ready else "DRAFT — downloadable-asset smoke tests pending"
    body += f"\n\n---\n**{status}**\n\nRevision: [`{commit}`](https://github.com/{repository}/commit/{commit}).\n"
    body += f"\nValidation run: https://github.com/{repository}/actions/runs/{os.environ.get('GITHUB_RUN_ID', '')}\n"
    if ready:
        body += "\nAll native downloadable-asset smoke tests passed for the hashes below.\n\n| Asset | SHA-256 |\n| --- | --- |\n"
        body += "".join(f"| {asset['name']} | `{asset['sha256']}` |\n" for asset in assets)
        body += "\nThis draft is not public. Review the exact revision's CI, Nix result, and migration notes before publishing.\n"
    return body


def edit_notes(repository, tag, body):
    with tempfile.TemporaryDirectory(prefix="typsite-release-notes-") as directory:
        path = Path(directory) / "notes.md"
        path.write_text(body, encoding="utf-8")
        gh("release", "edit", tag, "--repo", repository, "--notes-file", path)


def stage(repository_root, repository, commit, run, directory):
    report = manifest(directory, commit, version(repository_root))
    tag = f"{report['version']}-{run}"
    body = notes(repository_root, repository, commit)
    release = find_release(repository, tag)
    if release is None:
        # Fail closed if a pre-existing tag points at a different revision.
        # A run-number tag is workflow-owned; never repurpose a public tag.
        refs = json.loads(gh("api", f"repos/{repository}/git/matching-refs/tags/{quote(tag, safe='')}"))
        if any(ref["ref"] == f"refs/tags/{tag}" for ref in refs):
            raise ValueError("Refusing to reuse a pre-existing release tag")
        with tempfile.TemporaryDirectory(prefix="typsite-draft-") as temporary:
            path = Path(temporary) / "notes.md"
            path.write_text(body, encoding="utf-8")
            gh("release", "create", tag, "--repo", repository, "--target", commit,
               "--draft", "--title", f"Typsite {report['version']} — validation pending", "--notes-file", path)
        release = find_release(repository, tag)
    require_draft(release, commit)
    require_matching_tag(repository, tag, commit)
    edit_notes(repository, tag, body)
    gh("release", "edit", tag, "--repo", repository,
       "--title", f"Typsite {report['version']} — validation pending")
    files = [directory / name for name in TARGETS.values()]
    files += [directory / "release-manifest.json", directory / "SHA256SUMS"]
    checked_draft(repository, tag, commit)
    gh("release", "upload", tag, "--repo", repository, "--clobber", *files)
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a", encoding="utf-8") as file:
            file.write(f"tag={tag}\n")
    print(f"Staged draft {tag} for {commit}; no assets published")


def download(repository, tag, commit, asset, directory):
    checked_draft(repository, tag, commit)
    directory.mkdir(parents=True, exist_ok=True)
    gh("release", "download", tag, "--repo", repository, "--pattern", asset,
       "--pattern", "release-manifest.json", "--dir", directory, "--clobber")


def check_evidence(report, directory):
    evidence = [json.loads(path.read_text(encoding="utf-8")) for path in directory.rglob("smoke.json")]
    by_name = {entry["binary"]: entry for entry in evidence}
    if len(evidence) != len(TARGETS) or len(by_name) != len(TARGETS):
        raise ValueError("Expected exactly one successful smoke report per native target")
    for asset in report["assets"]:
        entry = by_name.get(asset["name"])
        pdf = entry.get("pdf") if entry is not None else None
        if (entry is None or entry["sha256"] != asset["sha256"]
                or entry["version"] != report["version"] or entry["provenance"] != asset
                or set(entry["passed"]) != REQUIRED_STAGES or entry["pages"] < 14
                or not isinstance(pdf, dict) or not isinstance(pdf.get("sha256"), str)
                or not re.fullmatch(r"[0-9a-f]{64}", pdf["sha256"])
                or not isinstance(pdf.get("size"), int) or pdf["size"] <= 1024):
            raise ValueError(f"Invalid downloaded-asset smoke evidence for {asset['name']}")
    return evidence


def ready(repository_root, repository, tag, commit, directory, evidence_directory):
    report = manifest(directory, commit, version(repository_root))
    check_evidence(report, evidence_directory)
    checked_draft(repository, tag, commit)
    # Recheck that the release still contains exactly the hashes that were tested.
    with tempfile.TemporaryDirectory(prefix="typsite-draft-verification-") as temporary:
        path = Path(temporary)
        gh("release", "download", tag, "--repo", repository, "--dir", path)
        expected_files = set(TARGETS.values()) | {"release-manifest.json", "SHA256SUMS"}
        if {file.name for file in path.iterdir()} != expected_files:
            raise ValueError("Draft release contains an unexpected or missing asset")
        downloaded = json.loads((path / "release-manifest.json").read_text(encoding="utf-8"))
        if downloaded != report:
            raise ValueError("Draft assets changed after smoke testing")
        for name in TARGETS.values():
            verify(path / name, path / "release-manifest.json", commit, report["version"])
        if (path / "SHA256SUMS").read_bytes() != (directory / "SHA256SUMS").read_bytes():
            raise ValueError("Draft checksum listing differs from validated build metadata")
    checked_draft(repository, tag, commit)
    edit_notes(repository, tag, notes(repository_root, repository, commit, ready=True, assets=report["assets"]))
    gh("release", "edit", tag, "--repo", repository,
       "--title", f"Typsite {report['version']} — validated draft")
    summary = f"Validated draft {tag}: https://github.com/{repository}/releases\nRevision: {commit}\nManual publication required; no assets are public.\n"
    print(summary)
    if output := os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(output, "a", encoding="utf-8") as file:
            file.write(summary)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("stage", "download", "ready"))
    parser.add_argument("--repository", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--run", type=int)
    parser.add_argument("--tag")
    parser.add_argument("--asset", choices=TARGETS.values())
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--evidence", type=Path)
    args = parser.parse_args()
    checked_commit(args.commit)
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", args.repository):
        parser.error("repository must be OWNER/REPO")
    root = Path(__file__).resolve().parent.parent
    if args.command == "stage":
        if args.run is None or args.run < 1:
            parser.error("stage requires a positive --run")
        stage(root, args.repository, args.commit, args.run, args.directory)
    elif args.command == "download":
        if not args.tag or not args.asset:
            parser.error("download requires --tag and --asset")
        download(args.repository, args.tag, args.commit, args.asset, args.directory)
    else:
        if not args.tag or not args.evidence:
            parser.error("ready requires --tag and --evidence")
        ready(root, args.repository, args.tag, args.commit, args.directory, args.evidence)


if __name__ == "__main__":
    main()
