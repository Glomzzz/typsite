#!/usr/bin/env python3
"""Package locked-build binaries and verify the exact assets used for smoke tests.

This script performs no network requests and never publishes a release. Release
creation/download is handled by the workflow; all provenance is content-checked.
"""

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import tomllib


TARGETS = {
    "x86_64-pc-windows-msvc": "typsite-x86_64-pc-windows-msvc.exe",
    "x86_64-unknown-linux-gnu": "typsite-x86_64-unknown-linux-gnu",
    "aarch64-apple-darwin": "typsite-aarch64-apple-darwin",
}


def version(repository):
    return tomllib.loads((repository / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]


def checked_commit(commit):
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("commit must be the exact 40-character revision SHA")
    return commit


def file_info(path):
    data = path.read_bytes()
    if not data:
        raise ValueError(f"Empty release asset: {path}")
    return {"sha256": hashlib.sha256(data).hexdigest(), "size": len(data)}


def package(binary, target, commit, repository, output):
    checked_commit(commit)
    output.mkdir(parents=True, exist_ok=True)
    destination = output / TARGETS[target]
    shutil.copy2(binary, destination)
    metadata = {
        "name": destination.name,
        "target": target,
        "commit": commit,
        "version": version(repository),
        **file_info(destination),
    }
    destination.with_name(destination.name + ".json").write_text(
        json.dumps(metadata, indent=2) + "\n", encoding="utf-8"
    )
    return metadata


def manifest(directory, commit, expected_version):
    checked_commit(commit)
    assets = []
    for target, name in TARGETS.items():
        asset = directory / name
        metadata = json.loads(asset.with_name(name + ".json").read_text(encoding="utf-8"))
        expected = {
            "name": name, "target": target, "commit": commit,
            "version": expected_version, **file_info(asset),
        }
        if metadata != expected:
            raise ValueError(f"Release provenance mismatch for {name}")
        assets.append(metadata)
    result = {"commit": commit, "version": expected_version, "assets": assets}
    (directory / "release-manifest.json").write_text(
        json.dumps(result, indent=2) + "\n", encoding="utf-8"
    )
    (directory / "SHA256SUMS").write_text(
        "".join(f"{asset['sha256']}  {asset['name']}\n" for asset in assets), encoding="utf-8"
    )
    return result


def verify(binary, manifest_path, commit, expected_version):
    checked_commit(commit)
    report = json.loads(manifest_path.read_text(encoding="utf-8"))
    if report["commit"] != commit or report["version"] != expected_version:
        raise ValueError("Release manifest is for a different commit or version")
    expected_names = set(TARGETS.values())
    names = [asset["name"] for asset in report["assets"]]
    if len(names) != len(expected_names) or set(names) != expected_names:
        raise ValueError("Release manifest must contain exactly the supported asset set")
    for asset in report["assets"]:
        if TARGETS.get(asset["target"]) != asset["name"]:
            raise ValueError("Release asset target/name mismatch")
        if asset["commit"] != commit or asset["version"] != expected_version:
            raise ValueError("Release asset provenance mismatch")
    asset = next((asset for asset in report["assets"] if asset["name"] == binary.name), None)
    if asset is None or file_info(binary) != {key: asset[key] for key in ("sha256", "size")}:
        raise ValueError(f"Downloaded asset checksum/size mismatch: {binary.name}")
    return asset


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    pack = subparsers.add_parser("package")
    pack.add_argument("--binary", type=Path, required=True)
    pack.add_argument("--target", choices=TARGETS, required=True)
    pack.add_argument("--commit", required=True)
    pack.add_argument("--output", type=Path, required=True)
    collect = subparsers.add_parser("manifest")
    collect.add_argument("--directory", type=Path, required=True)
    collect.add_argument("--commit", required=True)
    args = parser.parse_args()
    repository = Path(__file__).resolve().parent.parent
    if args.command == "package":
        result = package(args.binary, args.target, args.commit, repository, args.output)
    else:
        result = manifest(args.directory, args.commit, version(repository))
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
