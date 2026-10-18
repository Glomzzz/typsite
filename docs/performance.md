# Build parallelism and measurements

## What is parallel

Typsite already used Rayon for article-body/reference preparation and final page
rendering. The correctness-first build-pipeline refactor left raw Typst export
and article parsing sequential while establishing reliable cache/checkpoint,
generation-check, and publication behavior. This follow-up removes those serial
bottlenecks without persisting derived graph state or introducing a scheduler.

- **Filesystem snapshots:** sorted, validated discovery followed by independent
  file hashing with Rayon. Errors/results are collected in discovery order.
- **Cache validation:** raw artifact hashes checked in parallel. Shared external
  imports/packages are hashed once per pre-export snapshot, not per importer.
- **Typst exports:** a separate bounded Rayon pool for dirty articles. Each worker
  owns its pending artifact, verifies dependency generations, and atomically
  replaces only its own successful raw export.
- **Parsing:** build the complete slug registry first, then share immutable
  registry/configuration across independent Rayon article parses. Fold results
  and diagnostics in source-path order.
- **Rendering:** retain the existing Rayon body and page-schema passes with their
  relationship/reference barrier. Metadata remains dependency-ordered.
- **Published-output checks:** read-only output fingerprint comparisons use Rayon
  after validating all paths and ownership metadata.

Directory traversal/validation, package installation, graph resolution,
dependency-ordered metadata, manifest/checkpoint changes, and publication
mutations remain ordered. Stale deletion, file/directory transitions, and output
ownership are not safe targets for indiscriminate `par_iter()` replacement.
Output-plan name validation uses ordered ancestor/descendant lookups instead of
an all-pairs scan; export targets are collision-checked before workers start.

## Resource limits

```sh
typsite compile             # automatic, at most 4 concurrent article exports
typsite compile --jobs 4    # explicit upper bound; -j 4 is equivalent
typsite compile --jobs 1    # export articles sequentially
```

The automatic export limit is `min(available CPUs, Rayon pool size, 4)`. An
explicit positive `--jobs N` overrides the article count; actual workers never
exceed the number of dirty articles. Typst itself receives
`--jobs max(1, CPU_budget / concurrent_articles)` so concurrent subprocesses do
not each spawn a full machine's compilation worker pool. This bounds compilation
workers, not every OS/library helper thread. Very large explicit job limits can
still oversubscribe CPUs and consume substantial memory.

`--jobs` controls export concurrency, not all Rayon stages. `RAYON_NUM_THREADS`
tunes the ordinary Rayon pool and influences the automatic CPU budget. The
library equivalent is `Compiler::with_jobs(NonZeroUsize)`; injected runners must
support concurrent `export` calls to distinct output paths. `ExportRequest.jobs`
is the internal Typst subprocess worker budget.

Package preparation finishes before workers start. Workers return independent
results; all join before the caller updates the export checkpoint. A failed
worker leaves its old artifact intact but invalid, successful siblings remain
reusable, and publication does not proceed on failed exports. There is no
fail-fast cancellation of in-flight Typst children. Changing `--jobs` alone is
not a content change and does not invalidate warm caches or reload preview.

## Local measurements

Linux, Intel Core Ultra 9 185H (22 logical CPUs), Rust 1.94.0 release build,
Typst 0.15.1. Baseline: release binary from commit `3506e7ed`, before this
parallelism change. Median of three runs per scenario; no concurrent test/build workload.
Fixture: all 14 demo articles plus 64 independent generated articles with code,
math, footnotes, and repeated paragraphs. Each run starts with a cold Typsite
cache and isolated package namespaces; OS filesystem caches are not flushed.

| Scenario | Before (serial export/parse) | After (automatic, 4 exports) | Speedup |
| --- | ---: | ---: | ---: |
| Cold build, 78 exports | 12.84 s | 4.68 s | 2.74× |
| Warm no-op | 0.109 s | 0.089 s | 1.22× |
| Template edit, 78 pages rewritten | 1.039 s | 1.052 s | 0.99× |
| One article edit, 1 export | 0.319 s | 0.276 s | 1.16× |

The new binary with `--jobs 1` took 13.27 s for the cold fixture. Concurrency is
the main improvement; publication remains intentionally serial, and short warm
build differences are small enough to be sensitive to noise (the final template
run was slightly slower, not a claimed improvement). These numbers are
not universal speed guarantees. All output hashes and export/render/write/remove
counts matched between the baseline, automatic mode, and `--jobs 1` in every
scenario. Correctness tests additionally compare parallel incremental output
with clean builds, exercise actual overlapping workers and bounded concurrency,
source/dependency races, failure recovery, and stable diagnostic order.

## Reproduce

Build each revision in release mode and keep the binaries outside the checkout
before switching revisions. The script takes its fixture sources from the
**current** repository, so compare both binaries against the same source tree:

```sh
cargo build --release --locked
# Preserve the pre-change release binary separately, for example:
# cp target/release/typsite /tmp/typsite-before

python3 scripts/benchmark-build.py --binary /tmp/typsite-before \
  --label before --output /tmp/typsite-before.json
python3 scripts/benchmark-build.py --binary target/release/typsite \
  --label after --output /tmp/typsite-after.json
python3 scripts/benchmark-build.py --binary target/release/typsite \
  --jobs 1 --label jobs-1 --output /tmp/typsite-jobs-1.json
```

The dependency-free Python script uses temporary directories, child-only
environment settings, bundled/embedded fonts, a fixed timestamp, and no preview
package downloads. It records individual timings, medians, counts, and output
hashes. Use `--pages` / `--runs` to vary the workload; compare scenario hashes as
well as timings. It does not alter cwd/HOME or normal installed packages.
