# Build-pipeline refactor

Delivered in **Typsite 0.1.8**. User-facing migration guides are bundled as Typst
pages in the initialized demo:

- [English: Migrating to Typsite 0.1.8](../resources/root/en/migrate-to-018.typ)
- [中文：迁移到 Typsite 0.1.8](../resources/root/migrate-to-018.typ)

The executable version is 0.1.8; the bundled Typst package keeps the existing
`@local/typsite:0.1.0` import identity.

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
   **Done.** The runner uses direct, portable argument passing and current Typst
   JSON dependency metadata. Latest stable Typst compatibility is a requirement, not
   a best-effort extra; 0.15.1 was verified against GitHub's latest stable release
   and tested locally with the real executable. CI is configured for both 0.15.1
   and rolling latest stable on Linux, and latest stable on Windows/macOS. The offline
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
   generated files without deleting unrelated output files. Its paths use `/`
   separators on all platforms; legacy Windows-native manifests are readable. `.typsite.lock`
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
   now exercises the complete engine as well as raw export. Follow-up recovery
   tests exercise concurrent compiler instances, build locks, corrupt ownership
   metadata, file/directory transitions, and edits racing Typst export. Dependency
   fingerprints are checked on both sides of export; newly discovered external
   dependencies require one verification export. Input-generation races discard
   their export checkpoint rather than leaving incorrectly current cached HTML.
   Engine-binary changes invalidate publication, and true no-ops do not even
   rewrite the cache manifest.
6. Isolate blocking preview work, broadcast revisions, validate request paths,
   and add portable integration tests and CI. **Done.** Build and HTTP work use
   blocking workers, shutdown awaits in-flight work, and a monotonically
   increasing revision is visible to every browser rather than consumed by one
   client. The preview server handles decoded paths, query strings, pretty
   URLs, MIME types, HEAD requests, disconnected clients, and output symlinks.
   No-op and failed builds do not increment revisions. Changes from a partial
   failed publication stay pending until a successful repair, including a repair
   that only fixes the cache checkpoint and rewrites no page bytes. Polling remains
   intentionally simple (500 ms); no filesystem watcher or dependency task
   scheduler is introduced. Tokio's unstable APIs and custom compiler flags
   are no longer needed; release/deploy workflows use stable Rust.

Each step is independently tested and committed with a Conventional Commit.
The full test matrix and any remaining limitations must be documented here as
implementation progresses. Keep performance optimization separate from the
correctness work.

## Intentional validation changes

Malformed schemas, rewrite rules, themes, syntax definitions, and syntax metadata
now fail configuration loading, even when unused, rather than logging an error
and pretending the configuration loaded successfully. Duplicate resource names
are rejected rather than selected by parallel iteration order. HTML template
`<head>`/`<body>` envelopes must be present and balanced (self-closing empty
blocks are accepted); missing blocks no longer silently produce empty fragments.
File discovery is sorted and fallible, and treats spaces and glob metacharacters
in directory names literally. The real-Typst fixture covers these paths too.
Package preparation belongs to the executable runner, so injected runners never
install packages. Explicit namespace installation supports isolated builds and
tests. The default CLI keeps installing bundled/custom `@local` packages for
compatibility, but honors `TYPST_PACKAGE_PATH` / `TYPST_PACKAGE_CACHE_PATH`.
Package names/versions cannot traverse directories; duplicate identities and
filesystem errors are failures rather than skipped entries. Namespace locks
prevent concurrent installers from corrupting one another. Missing local
package installations are repaired before an invalid export is retried.

CLI paths may be absolute or relative, including paths/executable names with
spaces. Relative executable, font, and package environment paths are resolved
before changing Typst's working directory. Root resolution follows symlinks
before interpreting parent components.
`clean` validates both roots before deleting, protects source/configuration and
repository roots, and shares build locks without unlinking their inodes.
Unmarked legacy cache/output directories require `clean --force`; `--input` and
`--config` identify protected roots when using nondefault project directories.
These safeguards intentionally reject previously unsafe cleanup commands.

Self-parent relationships and recursive metadata are errors; ordinary citation
cycles (including a self-citation) remain valid. Deleting the last article still
cleans its output when a default parent is configured. Malformed SVG scaling
produces a diagnostic instead of a slicing panic. The latest-Typst smoke test
includes SVG links and footnote references as well as shipped page templates.

## RSS and sitemap

`[site].base_url` enables deterministic RSS 2.0 and sitemap generation. Blank
base URLs keep both disabled; blank output paths disable each independently,
and `rss_limit = 0` disables RSS. The new `rss_content` setting accepts
`"summary"` (default) or `"full"`. Summary mode uses plain text from the first
nonempty description/summary/abstract metadata. Full mode additionally exports
composed article bodies, not page schema/navigation/sidebar/footers, using the
standard `content:encoded` namespace. Articles are selected only from actual
**direct embeds** of pages with boolean metadata `rss = true` (`#rss(true)` in
Typst). Marked containers are always excluded from items, even when another
container embeds them; the marker defaults to false and is not inherited.
Multiple containers contribute a deduplicated union, sorted/limited afterward.
Nested embeds, citations, parent relationships, or graph-only child declarations
do not select separate items. No containers means an empty enabled feed.
`#rss(false)` removes a container marker, not another container's item selection.
Sitemap selection remains independent and includes every compiled article.

Ordinary link/image URLs become absolute;
SVG-local glyph references are preserved and scripts are omitted. This is not
a general HTML sanitizer or a replacement for trusted authoring inputs, and
feed readers' SVG/MathML/CSS/interactive-content support varies.

Every rendered page gets an autodiscovery link only while RSS is enabled. Feeds
have an Atom self-link, stable permalink GUIDs, XML-safe titles/summaries/content,
and RFC 2822 dates. Date-only/local timestamps use UTC; RFC 3339/RFC 2822 offsets
are normalized to UTC. Invalid dates are omitted, never emitted verbatim.
Full timestamps drive stable newest-first sorting; the latest metadata date,
not wall-clock build time, supplies `lastBuildDate`. Site URLs are validated as
absolute HTTP(S) URLs, and literal filename characters are URL-encoded.

Feed files share the output plan, collision checks, atomic writes, ownership
cleanup, and no-op behavior with pages/assets. Changing mode/path/limits needs
no Typst exports. XML-parsed integration tests cover both modes, discovery,
dates, limits, Unicode URLs, cleanup, failures, and clean-build equivalence.
The bilingual Typst homepages and 0.1.8 migration guides document configuration.

## Verification

Verified locally on Linux with Rust 1.94.0 and real Typst 0.15.1 (`9dfd3a08`):

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --all-targets --locked --offline
cargo test --test typst_latest --locked --offline -- --ignored
cargo test --workspace --doc --locked --offline
cargo build --release --locked --offline
```

The default suite passes **105 unit/integration tests**; the three real-Typst
tests are ignored by default and pass when explicitly enabled. The fixtures
compare full incremental/clean output snapshots, include independent compiler
instances, and never change process cwd, HOME, or normal installed packages.
CLI fixtures use child-only environment variables and a compiled native stub;
that stub validates invocation and exit behavior, not Typst compatibility.
The Unix watch-process test verifies multi-client revisions, no-op polls,
last-good pages during failure, repair, and graceful Ctrl-C shutdown.

The complete-demo test uses the real CLI to initialize the embedded resources,
compile **all 14 demo pages**, check local file links, same-page anchors, and
assets, verify a warm no-op, edit an article, and compare the incremental site
with a clean rebuild. It uses
isolated package/cache namespaces, a fixed timestamp, and no system fonts. The
updated flowchart uses built-in Typst drawing instead of a preview package, and
unused `frame-it` setup was removed; no preview-package downloads are needed.
The support table now uses real SVG footnote references rather than decorative
superscripts, so its footnote return links have actual targets. A legacy `.typ`
link in the English 0.1.7 guide now uses a site citation and resolves to HTML.
The demo includes both 0.1.8 migration guides and links them from each homepage.
The full-demo fixture enables RSS and checks its four selected items and all
page discovery links: the two marked homepages select their directly embedded
article guide and Typst introduction; homepages, migrations, and examples are
not feed items. It switches from summary to full mode without re-exporting
articles, parses generated XML, and includes feeds in incremental/clean checks.
The real-Typst helper fixture checks both the false marker and rejection of
non-boolean arguments. Portable RSS tests cover deduplication, nested embeds,
marked-container exclusion, stale selections, metadata validation, and recovery.
External web fonts and music embeds are browser dependencies, not build inputs.

CI includes strict formatting/Clippy, an MSRV 1.89 compile check, and real-Typst
Linux/Windows/macOS jobs. Remote CI, Windows/macOS execution, and the MSRV toolchain
have **not** been run locally; no remote push was made to trigger those jobs.

## Deliberate limitations

- Dirty builds reparse cached raw Typst HTML and recompose **all** pages. This
  conservatively handles removed dependency edges; affected-page minimization
  and owned parsed-IR persistence remain future work. Existing borrowed IR and
  serializer types are retained, but the engine never reloads derived graph or
  rendered-fragment caches from them.
- Publication is atomic per file, not a whole-site or power-loss transaction.
  Ownership metadata is not disposable: corrupt metadata refuses unsafe guesses;
  deleting it explicitly gives up ownership of old files. Normal builds never
  remove unowned files, while an explicit `clean` clears selected root contents.
- Watch polls every 500 ms. Shutdown waits for the current Typst export; there is
  no export timeout or child-process cancellation policy yet.
- Fingerprint checks and bounded verification exports detect ordinary racing
  edits, but do not provide an atomic snapshot of an actively changing project.
- Reproducibility assumes stable external inputs. Use project fonts, isolated
  package paths, and `SOURCE_DATE_EPOCH` for hermetic builds. System-font installs,
  time-dependent source, and undeclared environment/network effects are not a
  general hermetic dependency model; discard the build cache when necessary.
- The default CLI package namespace remains shared for compatibility. Use
  per-project `TYPST_PACKAGE_PATH` or the runner's explicit namespace for isolation.
- Older Typst releases are not a compatibility target. Latest stable is required
  and the rolling CI job is intended to catch future CLI/export changes.

## Baseline

Before semantic changes: 56 tests passed; build and Clippy completed (with lint
warnings); formatting differed in 31 files. Isolated stub-backed smoke checks
reproduced stale backlinks after citation removal, successful exit after output
write failure, and `--input content` forwarding the literal `--root root`.
These checks did not establish compatibility with real Typst releases.
