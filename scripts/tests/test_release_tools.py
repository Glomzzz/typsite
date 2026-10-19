import copy
import json
import shutil
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


SCRIPTS = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS))
import release_artifacts as artifacts
import release_draft as draft
import smoke_binary


SHA = "a" * 40
VERSION = "0.1.8"


class ReleaseToolsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="typsite-release-tools-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "Cargo.toml").write_text(f'[package]\nversion = "{VERSION}"\n', encoding="utf-8")
        notes = self.root / f"docs/releases/{VERSION}.md"
        notes.parent.mkdir(parents=True)
        notes.write_text("Migration notes", encoding="utf-8")
        self.dist = self.root / "dist"
        for target in artifacts.TARGETS:
            binary = self.root / f"input-{target}"
            binary.write_bytes(b"binary fixture: " + target.encode())
            artifacts.package(binary, target, SHA, self.root, self.dist)
        self.report = artifacts.manifest(self.dist, SHA, VERSION)

    def test_gh_decodes_utf8_without_using_windows_locale(self):
        release = {"draft": True, "target_commitish": SHA, "tag_name": "tag", "body": "发布说明"}
        payload = json.dumps([[release]], ensure_ascii=False).encode("utf-8")

        def check_output(*arguments, **options):
            return payload.decode(options.get("encoding", "cp1252"))

        with mock.patch.object(draft.subprocess, "check_output", side_effect=check_output) as command:
            self.assertEqual(draft.find_release("owner/repo", "tag"), release)
        command.assert_called_once_with(
            ["gh", "api", "--paginate", "--slurp", "repos/owner/repo/releases?per_page=100"],
            text=True, encoding="utf-8",
        )

    def test_complete_manifest_and_downloaded_asset_verification(self):
        self.assertEqual(len(self.report["assets"]), 3)
        for asset in self.report["assets"]:
            checked = artifacts.verify(self.dist / asset["name"], self.dist / "release-manifest.json", SHA, VERSION)
            self.assertEqual(checked, asset)
        self.assertEqual(len((self.dist / "SHA256SUMS").read_text().splitlines()), 3)

    def test_tampered_binary_and_wrong_revision_or_version_fail_closed(self):
        binary = self.dist / next(iter(artifacts.TARGETS.values()))
        with self.assertRaises(ValueError):
            artifacts.verify(binary, self.dist / "release-manifest.json", "b" * 40, VERSION)
        with self.assertRaises(ValueError):
            artifacts.verify(binary, self.dist / "release-manifest.json", SHA, "0.0.0")
        binary.write_bytes(b"tampered asset")
        with self.assertRaises(ValueError):
            artifacts.verify(binary, self.dist / "release-manifest.json", SHA, VERSION)
        with self.assertRaises(ValueError):
            artifacts.manifest(self.dist, SHA, VERSION)

    def test_missing_or_mixed_provenance_is_not_a_complete_release(self):
        asset = self.report["assets"][0]
        metadata = self.dist / (asset["name"] + ".json")
        data = json.loads(metadata.read_text())
        data["commit"] = "b" * 40
        metadata.write_text(json.dumps(data))
        with self.assertRaises(ValueError):
            artifacts.manifest(self.dist, SHA, VERSION)
        metadata.unlink()
        with self.assertRaises(FileNotFoundError):
            artifacts.manifest(self.dist, SHA, VERSION)

    def test_duplicate_or_unsupported_assets_in_manifest_are_rejected(self):
        binary = self.dist / self.report["assets"][0]["name"]
        report = copy.deepcopy(self.report)
        report["assets"][1] = report["assets"][0]
        path = self.dist / "release-manifest.json"
        path.write_text(json.dumps(report))
        with self.assertRaises(ValueError):
            artifacts.verify(binary, path, SHA, VERSION)

    def test_only_exact_draft_can_be_modified(self):
        for release in (None, {"draft": False, "target_commitish": SHA},
                        {"draft": True, "target_commitish": "main"},
                        {"draft": True, "target_commitish": "b" * 40}):
            with self.assertRaises(ValueError):
                draft.require_draft(release, SHA)
        draft.require_draft({"draft": True, "target_commitish": SHA}, SHA)

    def test_public_release_rerun_never_uploads_or_mutates(self):
        release = {"draft": False, "target_commitish": SHA}
        with mock.patch.object(draft, "find_release", return_value=release), mock.patch.object(draft, "gh") as gh:
            with self.assertRaises(ValueError):
                draft.stage(self.root, "owner/repo", SHA, 42, self.dist)
            gh.assert_not_called()

    def test_existing_tag_is_not_repurposed(self):
        refs = [{"ref": f"refs/tags/{VERSION}-42"}]
        with mock.patch.object(draft, "find_release", return_value=None), mock.patch.object(draft, "gh", return_value=json.dumps(refs)) as gh:
            with self.assertRaises(ValueError):
                draft.stage(self.root, "owner/repo", SHA, 42, self.dist)
            self.assertEqual(gh.call_count, 1)
            self.assertEqual(gh.call_args.args[0], "api")

    def test_staging_uses_draft_only_and_uploads_exact_expected_assets(self):
        release = {"draft": True, "target_commitish": SHA}
        with mock.patch.object(draft, "find_release", side_effect=[None, release, release]), mock.patch.object(draft, "gh", return_value="[]") as gh:
            draft.stage(self.root, "owner/repo", SHA, 42, self.dist)
        calls = [call.args for call in gh.call_args_list]
        create = next(call for call in calls if call[:2] == ("release", "create"))
        self.assertIn("--draft", create)
        self.assertEqual(create[create.index("--target") + 1], SHA)
        for call in calls:
            if call[:2] == ("release", "edit"):
                self.assertFalse(any(str(argument).startswith("--draft") for argument in call),
                                 "editing notes/title must not change publication state")
        upload = next(call for call in calls if call[:2] == ("release", "upload"))
        self.assertEqual({path.name for path in upload if isinstance(path, Path)},
                         set(artifacts.TARGETS.values()) | {"release-manifest.json", "SHA256SUMS"})

    def write_evidence(self):
        directory = self.root / "evidence"
        for index, asset in enumerate(self.report["assets"]):
            output = directory / str(index) / "smoke.json"
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(json.dumps({
                "binary": asset["name"], "sha256": asset["sha256"], "version": VERSION,
                "provenance": asset, "pages": 14, "passed": sorted(draft.REQUIRED_STAGES),
                "pdf": {"sha256": "b" * 64, "size": 2048},
            }))
        return directory

    def test_all_native_smoke_evidence_is_required(self):
        directory = self.write_evidence()
        self.assertEqual(len(draft.check_evidence(self.report, directory)), 3)
        (directory / "0/smoke.json").unlink()
        with self.assertRaises(ValueError):
            draft.check_evidence(self.report, directory)

    def test_wrong_hash_or_incomplete_smoke_cannot_be_marked_ready(self):
        for key, value in (("sha256", "wrong"), ("passed", ["warm no-op"]), ("pages", 1)):
            directory = self.write_evidence()
            path = directory / "0/smoke.json"
            data = json.loads(path.read_text())
            data[key] = value
            path.write_text(json.dumps(data))
            with self.assertRaises(ValueError):
                draft.check_evidence(self.report, directory)

    def test_html_only_or_invalid_pdf_evidence_cannot_be_marked_ready(self):
        for pdf in (None, {}, {"sha256": "invalid", "size": 2048},
                    {"sha256": "b" * 64, "size": 12}):
            directory = self.write_evidence()
            path = directory / "0/smoke.json"
            data = json.loads(path.read_text())
            data["pdf"] = pdf
            path.write_text(json.dumps(data))
            with self.assertRaises(ValueError):
                draft.check_evidence(self.report, directory)
        directory = self.write_evidence()
        path = directory / "0/smoke.json"
        data = json.loads(path.read_text())
        data["passed"] = [stage for stage in data["passed"] if not stage.startswith("PDF")]
        path.write_text(json.dumps(data))
        with self.assertRaises(ValueError):
            draft.check_evidence(self.report, directory)

    def test_pdf_validation_rejects_missing_and_partial_artifacts(self):
        path = self.root / "book/site.pdf"
        path.parent.mkdir()
        with self.assertRaises(FileNotFoundError):
            smoke_binary.read_pdf(path)
        for data in (b"not a PDF" * 200, b"%PDF-1.7\npartial"):
            path.write_bytes(data)
            with self.assertRaises(RuntimeError):
                smoke_binary.read_pdf(path)
        data = b"%PDF-1.7\n" + b"fixture" * 200
        path.write_bytes(data)
        self.assertEqual(smoke_binary.read_pdf(path), data)

    def test_pdf_smoke_records_every_required_stage_and_uses_offline_covers(self):
        project = self.root / "smoke project"
        (project / "root/lib").mkdir(parents=True)
        (project / ".typsite").mkdir()
        (project / ".typsite/options.toml").write_text('[layout]\nmodule = ""\n', encoding="utf-8")
        calls = []

        def compile_html():
            publish = project / "publish"
            publish.mkdir(exist_ok=True)
            data = '<html><header class="page-header">Header</header>Body<footer class="page-footer">Footer</footer></html>'
            (publish / "index.html").write_text(data, encoding="utf-8")
            return 0, 1, 1, 0

        def run(arguments, success=True, diagnostic=None):
            calls.append((list(arguments), success, diagnostic))
            if arguments == ["clean"]:
                shutil.rmtree(project / ".cache")
                shutil.rmtree(project / "publish")
                return "Cleaned"
            self.assertEqual(arguments[0], "pdf")
            for option, expected in (("--font-profile", "none"), ("--front-cover", "lib/release-front.typ"),
                                     ("--back-cover", "lib/release-back.typ")):
                self.assertEqual(arguments[arguments.index(option) + 1], expected)
            self.assertIn("--offline", arguments)
            self.assertEqual(arguments.count("--appendix"), 5)
            self.assertIn("Release PDF forest", (project / "root/lib/release-pdf.typ").read_text())
            self.assertIn("release-native-header", (project / "root/lib/release-pdf.typ").read_text())
            appendix = (project / "root/lib/release-appendix.typ").read_bytes()
            layout = (project / "root/lib/release-layout.typ").read_bytes()
            if not success:
                self.assertIn(b"Unsafe contextual heading", appendix)
                self.assertEqual(diagnostic, "cannot be normalized safely")
                return ""
            output = project / "book/site.pdf"
            cache = project / ".cache/pdf"
            cache.mkdir(parents=True, exist_ok=True)
            output.parent.mkdir(exist_ok=True)
            data = b"%PDF-1.7\n" + (appendix + layout) * 5
            artifact = cache / "artifact.pdf"
            if artifact.exists() and artifact.read_bytes() == data and output.read_bytes() == data:
                return "PDF unchanged (cached)."
            artifact.write_bytes(data)
            output.write_bytes(data)
            return "PDF written (compiled)."

        stages = []
        result = smoke_binary.smoke_pdf(project, run, compile_html, stages)
        self.assertEqual(set(stages), {stage for stage in draft.REQUIRED_STAGES if stage.startswith("PDF")})
        self.assertGreater(result["size"], 1024)
        self.assertRegex(result["sha256"], r"^[0-9a-f]{64}$")
        self.assertEqual(sum(not success for _, success, _ in calls), 1)
        self.assertTrue((project / "book/site.pdf").exists())

    def test_download_checks_draft_before_network_download(self):
        with mock.patch.object(draft, "find_release", return_value={"draft": False, "target_commitish": SHA}), mock.patch.object(draft, "gh") as gh:
            with self.assertRaises(ValueError):
                draft.download("owner/repo", "tag", SHA, next(iter(artifacts.TARGETS.values())), self.root / "download")
            gh.assert_not_called()

    def test_smoke_decodes_utf8_commands_and_diagnostics_without_windows_locale(self):
        binary = self.root / "typsite.exe"
        binary.write_bytes(b"binary fixture")

        def run(arguments, **options):
            success = arguments[-1] == "--version"
            payload = f"typsite {VERSION}" if success else "初始化失败"
            output = payload.encode("utf-8").decode(options.get("encoding", "cp1252"))
            return smoke_binary.subprocess.CompletedProcess(arguments, 0 if success else 1, output, "")

        def check_output(*arguments, **options):
            return "typst 0.15.1 测试".encode("utf-8").decode(options.get("encoding", "cp1252"))

        with mock.patch.object(smoke_binary.subprocess, "run", side_effect=run) as command, \
                mock.patch.object(smoke_binary.subprocess, "check_output", side_effect=check_output) as typst:
            with self.assertRaisesRegex(RuntimeError, "初始化失败"):
                smoke_binary.smoke(binary, VERSION, "typst")
        self.assertEqual(command.call_count, 2)
        for call in command.call_args_list + typst.call_args_list:
            self.assertEqual(call.kwargs["encoding"], "utf-8")

    def test_smoke_snapshot_includes_ownership_and_failures_are_errors(self):
        (self.root / "publish").mkdir()
        (self.root / "publish/.typsite-outputs.json").write_text("{}")
        self.assertIn(".typsite-outputs.json", smoke_binary.snapshot(self.root / "publish"))
        with self.assertRaises(RuntimeError):
            smoke_binary.require(False, "must fail")
        smoke_binary.require(True, "must pass")

    def test_ready_redownloads_and_checks_actual_bytes_without_publishing(self):
        evidence = self.write_evidence()
        release = {"draft": True, "target_commitish": SHA}

        def gh(*arguments):
            if arguments[:2] == ("release", "download"):
                destination = Path(arguments[arguments.index("--dir") + 1])
                for name in set(artifacts.TARGETS.values()) | {"release-manifest.json", "SHA256SUMS"}:
                    shutil.copy2(self.dist / name, destination / name)
            return "[]"

        with mock.patch.object(draft, "find_release", return_value=release), mock.patch.object(draft, "gh", side_effect=gh) as command:
            draft.ready(self.root, "owner/repo", "tag", SHA, self.dist, evidence)
        for call in command.call_args_list:
            self.assertNotIn("--draft=false", call.args)
            self.assertNotIn("--latest", call.args)
        self.assertTrue(any("validated draft" in str(call.args) for call in command.call_args_list))

    def test_ready_fails_if_downloaded_binary_changed_after_smoke(self):
        evidence = self.write_evidence()
        release = {"draft": True, "target_commitish": SHA}

        def gh(*arguments):
            if arguments[:2] == ("release", "download"):
                destination = Path(arguments[arguments.index("--dir") + 1])
                for name in set(artifacts.TARGETS.values()) | {"release-manifest.json", "SHA256SUMS"}:
                    shutil.copy2(self.dist / name, destination / name)
                (destination / next(iter(artifacts.TARGETS.values()))).write_bytes(b"changed after smoke")
            return "[]"

        with mock.patch.object(draft, "find_release", return_value=release), mock.patch.object(draft, "gh", side_effect=gh) as command:
            with self.assertRaises(ValueError):
                draft.ready(self.root, "owner/repo", "tag", SHA, self.dist, evidence)
        self.assertFalse(any(call.args[:2] == ("release", "edit") for call in command.call_args_list))

    def test_lightweight_and_annotated_tags_must_resolve_to_exact_commit(self):
        for target in ({"type": "commit", "sha": SHA}, {"type": "tag", "sha": "c" * 40}):
            ref = [{"ref": "refs/tags/test", "object": target}]
            responses = [json.dumps(ref)]
            if target["type"] == "tag":
                responses.append(json.dumps({"object": {"type": "commit", "sha": SHA}}))
            with mock.patch.object(draft, "gh", side_effect=responses):
                draft.require_matching_tag("owner/repo", "test", SHA)
        ref = [{"ref": "refs/tags/test", "object": {"type": "commit", "sha": "b" * 40}}]
        with mock.patch.object(draft, "gh", return_value=json.dumps(ref)):
            with self.assertRaises(ValueError):
                draft.require_matching_tag("owner/repo", "test", SHA)
        with mock.patch.object(draft, "gh", return_value="[]"):
            draft.require_matching_tag("owner/repo", "not-yet-tagged-draft", SHA)

    def test_short_revision_names_are_not_accepted_for_release_provenance(self):
        for commit in ("main", "a" * 7, "A" * 40, "a" * 41):
            with self.assertRaises(ValueError):
                artifacts.checked_commit(commit)


if __name__ == "__main__":
    unittest.main()
