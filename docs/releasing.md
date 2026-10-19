# Stable release gates

Local Linux test results are not evidence of Windows, macOS, or MSRV execution.
Every stable candidate needs green checks for the **exact revision** that
produced its binaries. Nothing in this document grants permission to push,
create a release, or publish one.

## Automated order

1. `check.yml` runs formatting/strict Clippy, release-tool unit tests, Rust 1.89
   all-target checking/tests with real Typst, Linux Typst 0.15.1 + rolling latest,
   Windows/macOS latest real-Typst tests, and the documented Nix build. Native,
   MSRV, and Nix binaries all run the HTML **and PDF** smoke harness. Real-Typst
   suites run serially to avoid transient inherited-lock contention while other
   tests spawn subprocesses. The
   `required` job fails unless every dependency succeeded; skipped/cancelled jobs
   are not accepted as green.
2. On pushes to `main`, `release.yml` calls that same reusable check workflow
   from the **same commit**, not the last green run of another revision. Every
   release build depends on its success. Set branch protection to require the
   aggregate check as well; workflow files cannot configure repository rules.
3. Native Linux x86_64, Windows x86_64, and macOS arm64 builds use
   `cargo build --release --locked --target ...`. Job-local artifacts include the
   target, package version, full source SHA, binary size, and SHA-256 checksum.
4. Only after all checks and builds succeed, the staging job creates an
   **unpublished draft**, using the existing `<version>-<run number>` tag style.
   It refuses pre-existing public releases/tags or mismatching source SHAs.
   `release-manifest.json` and `SHA256SUMS` describe exactly the three assets.
5. Each native runner **downloads its executable from the draft release**,
   verifies SHA/version/checksum/size, then runs `init → compile → edit → compile`
   against real latest Typst. The smoke harness also verifies every demo page,
   RSS, no-op mtimes, embedded-page updates, truthful failure/last-good repair,
   and incremental/clean equivalence. It then compiles a native book with ordered
   appendices, both covers, template and shared page furniture, native equations,
   images, and footnotes. Typst-side assertions check the document structure,
   cover positions, numbering, and header/footer counts without requiring PDF
   extraction tools. The PDF stages verify byte/mtime no-ops, appendix/layout
   edits, failed-build recovery, HTML ownership isolation, and clean-build byte
   equivalence. All package paths are temporary; remote fonts are disabled and
   `SOURCE_DATE_EPOCH` is fixed. Release tokens are removed from the executable's
   child environment.
6. The final `ready` job validates all smoke reports, redownloads/rechecks the
   asset manifest and binary checksums, and marks the **draft** validated.
   HTML-only reports are rejected: every platform must report all five PDF
   stages plus a nonempty PDF size and SHA-256. PDF hashes need not match across
   platforms; clean equivalence is checked within each runner. A
   failure leaves it unpublished; a public-release rerun is refused before
   mutation. Publication must not occur concurrently with validation; the
   script rechecks draft status but is not a server-side atomic release lock.
7. A maintainer reviews the exact-SHA workflow results, smoke evidence,
   checksums, and `docs/releases/0.1.8.md`, then manually publishes the draft.
   **No workflow step automatically publishes assets.** Do not bypass a failed
   gate with the GitHub release UI.

Draft downloads need a token with access to draft releases; the workflow grants
write access only to draft-staging/download/finalization jobs, not PR validation
or compilation. Checkout credentials are not persisted. Never pass release
secrets to a workflow running untrusted pull-request code.

## Local preflight

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo test --test typst_latest --test typst_pdf --locked -- --ignored --test-threads=1
python3 -m unittest discover -s scripts/tests -v
nix build --no-link --print-out-paths .#default
cargo build --release --locked
python3 scripts/smoke_binary.py --binary target/release/typsite
```

On Windows use `python` and `target/release/typsite.exe`. For a downloaded asset,
verify release provenance before executing it:

```sh
python3 scripts/smoke_binary.py --binary downloaded/typsite-x86_64-unknown-linux-gnu \
  --manifest downloaded/release-manifest.json --commit FULL_40_CHARACTER_SHA \
  --report smoke.json
```

The standalone smoke test does not compile Typsite and works with the Nix-built
or downloaded binary. It requires Python 3.11+ and real latest stable Typst. It
never changes the invoking process cwd/HOME or normal installed packages.

## Candidate evidence

Record the candidate SHA, Check workflow URL/conclusion, Release workflow URL,
Nix result, per-platform smoke-report artifacts (including `pdf.size` and
`pdf.sha256`), and exact binary checksums.
Changing code, lockfiles, resources, or workflow definitions creates a new
candidate: rerun the full matrix. A run configured in YAML but not observed
passing is **pending**, not validation.

The development implementation was tested on local Linux. Remote final-revision
CI and actual draft-asset downloads must be observed after an authorized push;
they are not implied by a local build or by the presence of these workflows.
