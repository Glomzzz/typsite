# Build-pipeline refactor

## Contract

This is a behavior-preserving refactor on `dev`, not a new authoring API.
Existing CLI arguments, project configuration, Typst helpers, templates, and
valid rendered content remain supported. Correcting stale output, false build
success, invalid paths, or nonterminating recursive graphs is intentional.

Internal caches are disposable and may change format. Prefer conservative
whole-page recomposition over fine-grained body/sidebar cache invalidation.
Keep Typst export incremental. Do not introduce a plugin framework, embedded
Typst compiler, template language replacement, or a multi-crate architecture.

The primary invariant is:

```
incremental(previous_site, edits) == clean_build(site_after_edits)
```

Required edit sequences include citations/embeds being added and removed,
metadata inheritance, article deletion/rename, template changes/deletion,
library/package changes, and recovery after failed compilation or output writes.
No-op builds must not rewrite outputs or notify preview clients.

## Architecture

A thin CLI and preview server call a library build engine. Project options and
rewrite registries belong to that project, never process-global mutable state.

```
discover -> export -> parse -> resolve graph -> render -> publish -> report
```

Keep source facts separate from derived relationships and render-time caches.
Build relationships from the current article set. Detect recursive embed and
parent graphs before lazy rendering; citation cycles are valid. Failed stages
must produce diagnostics, not fabricated empty content or successful reports.

Use a versioned cache manifest, stage-specific input fingerprints, atomic file
replacement, and managed output paths. Never advance an artifact's cache state
before that artifact is successfully written. Recover from missing/corrupt cache
entries without a manual clean.

## Delivery sequence

1. Normalize formatting separately from semantic changes. **Done.**
2. Extract a library, remove duplicated regression wrappers, and construct the
   built-in rewrite registry explicitly. **Done.** The existing 56-test count
   contained 20 duplicate wrappers; their underlying tests remain.
3. Make configuration project-local; verify independent compiler instances.
   **Done.** Options are immutable project-owned values, metadata carries its
   project's defaults, and tests run without a global lock. Concurrent metadata
   tests exercise different defaults and URL policies. Removed the unnecessary
   manual `Send` implementation; the compiler now verifies thread safety.
   Also made an absent `[site]` section use the documented field defaults.
4. Introduce a testable Typst runner and truthful structured build reports.
   The runner now uses direct, portable argument passing and current Typst JSON
   dependency metadata. Latest stable Typst compatibility is a requirement, not
   a best-effort extra; 0.15.1 was verified against GitHub's latest stable release
   and tested locally with the real executable. CI tests both 0.15.1 and rolling
   latest stable on Linux, and latest stable on Windows/macOS. The offline
   compatibility fixture uses isolated bundled packages and exercises metadata,
   headings, footnotes, code, MathML, SVG frames, and both inline/block math
   fallback. Fixed the bundled fallback helper's outdated inline call.
5. Replace persistent rendered-fragment/dependency caches with conservative
   rebuild planning, validated graph resolution, and recoverable publication.
   **Done.** A versioned manifest caches raw Typst HTML plus actual dependency
   hashes from current Typst's `--deps` output. Dirty builds parse source HTML
   anew, reconstruct relationships, validate embed/parent/metadata recursion,
   and render complete pages. The initial conservative planner recomposes all
   pages, not only a minimal affected subset; unchanged output bytes are never
   rewritten. Typst export remains selective, including imports not declared in
   `typst_lib`. No parsed-IR cache is persisted yet.

   Publication uses atomic per-file replacement and an ownership journal in
   `.typsite-outputs.json`; this survives cache deletion and safely removes stale
   generated files without deleting unrelated output files. `.typsite.lock`
   files provide cross-process build exclusion (Rust 1.89+). These names are
   reserved. Exports and publication have independent cache checkpoints.
   Failed export/parse/resolve/render stages retain the last successful site;
   publication failures may leave a mixture of complete old/new files, never a
   truncated file. They report failure and an ordinary rerun repairs the site.
   This is deliberately not an all-or-nothing whole-site deployment transaction.

   Six tests tied to the removed cache implementation were replaced by portable
   build-level coverage: citation removal, footer template isolation, malformed
   HTML, invalid roots, cache recovery, and publication failures, plus graph,
   asset, feed, dependency, rename, and no-op cases. The real latest-Typst test
   now exercises the complete engine as well as raw export.
6. Isolate blocking preview work, broadcast revisions, validate request paths,
   and add portable integration tests and CI.

Each step is independently tested and committed with a Conventional Commit.
The full test matrix and any remaining limitations must be documented here as
implementation progresses. Keep performance optimization separate from the
correctness work.

## Baseline

Before semantic changes: 56 tests passed; build and Clippy completed (with lint
warnings); formatting differed in 31 files. Isolated stub-backed smoke checks
reproduced stale backlinks after citation removal, successful exit after output
write failure, and `--input content` forwarding the literal `--root root`.
These checks did not establish compatibility with real Typst releases.
