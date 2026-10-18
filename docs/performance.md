# Build performance

## What changed

Rayon was already used for article-body/reference preparation and final page
rendering. The initial correctness refactor made export/parsing sequential; the
first parallelism pass removed that bottleneck but was still slower than the
actual 0.1.7 release in several cases. Comparing only against an intermediate
0.1.8 commit was not enough. This follow-up measures the latest tagged 0.1.7
source and addresses startup, shared dependency checks, and publication costs.

- **Verified early no-op:** hash actual source/configuration/package inputs, raw
  exports, external dependencies, and published files before parsing configuration.
  Every configuration-content change still triggers strict validation, including
  unused malformed themes/syntaxes. Typst identity probing overlaps filesystem
  reads; small file sets avoid initializing Rayon just to hash tiny files.
- **Highlighting-resource cache:** validated themes, custom syntax definitions,
  and syntax metadata are cached by content fingerprint, project root, and engine
  identity. A magic/version marker and checksum detect truncated/corrupt data.
  Syntax metadata is serialized separately because Syntect skips it in SyntaxSet.
- **Shared dependency metadata:** `manifest.bin` interns identical dependency
  sets. Source hashes are stored once per article, not repeated in its imports.
  Artifact/source/dependency hashes are still verified; old JSON/binary cache
  formats are disposable misses. The output ownership journal is separate.
- **Bounded concurrent exports:** prepare local packages first and hash their
  installed files before export. Workers own independent pending outputs; after
  joining a batch, shared dependencies are hashed once. Unknown/changing inputs
  require verification exports, with at most three attempts. No invalid artifact
  is published; successful siblings remain reusable after an ordinary failure.
- **Parallel publication:** validate the full plan and journal ownership first;
  remove stale owned paths in order, then sync/rename distinct output files in
  parallel. Fold every result in source order, including successful siblings of
  failures, and sync each changed parent directory before finalizing ownership.
  Published-file data syncing is retained, not removed to win a benchmark.
- **Less repeated work:** immutable-registry article parsing and page rendering
  use Rayon. Template substitutions avoid constructing automata for absent or
  single replacements. Path validation inspects shared managed ancestors once
  per checked set, and collision detection uses ordered lookups, not all pairs.
- **Speed-oriented release build:** `opt-level = 3` replaces size optimization.
  `build.rs` fingerprints code, bundled resources, Cargo files, compiler/target,
  and relevant codegen settings once at build time, avoiding an executable-file
  hash on every CLI invocation. The CLI uses one Tokio runtime thread; blocking
  compilation/HTTP work stays off that thread.

Graph resolution, dependency-ordered metadata, package preparation, stale
removals, and ownership/checkpoint mutations remain ordered. Independent file
writes are parallel, not the ownership protocol. This is data parallelism, not
a new dependency scheduler or a persisted graph/rendered-fragment cache.

## Resource limits and durability

```sh
typsite compile             # automatic available CPU/Rayon budget
typsite compile --jobs 4    # explicit maximum article processes; -j 4 works too
typsite compile --jobs 1    # sequential export; lower peak memory
```

The automatic export limit is `min(available CPUs, Rayon pool size, dirty
articles)`. The earlier fixed four-article cap is removed. Each Typst subprocess
receives `--jobs max(1, CPU_budget / concurrent_articles)` rather than each
spawning a full machine's compilation worker pool. This limits compilation
workers, not all OS/library threads. Very large explicit `--jobs` settings can
still oversubscribe CPUs and consume memory; reduce jobs on constrained systems.

`--jobs` controls article exports, not all Rayon stages. `RAYON_NUM_THREADS`
tunes the ordinary Rayon pool and influences the automatic CPU budget. The
library equivalent is `Compiler::with_jobs(NonZeroUsize)`. Runners must support
concurrent exports to distinct paths; `prepare` returns installed-resource roots
for pre-export fingerprinting. `ExportRequest.jobs` is the subprocess CPU budget.
All export workers finish before checkpoint/publication; shutdown does not
abandon active children. Changing jobs alone is not a content/cache change.

Disposable raw exports, highlighting data, and the build manifest use atomic
renames **without fsync**. After power loss, missing/corrupt cache data is a miss,
not trusted output. Published files retain `sync_data` before atomic rename, and
parent-directory syncs are batched on Unix. The ownership journal retains its
independent synced writes. This remains per-file atomic publication, **not** a
whole-site or universal power-loss transaction. A failed publication can leave
complete old/new files mixed; all results are joined/reported and reruns repair
through the ownership journal. The user explicitly chose to keep fsync durability
rather than remove it to improve cold timings.

## Comparison against actual 0.1.7

Baseline: latest tagged `v0.1.7-51`, commit **`5038f5bb`**, built unchanged from
source with its locked dependencies and required `RUSTFLAGS='--cfg tokio_unstable'`.
Both versions use Rust 1.94.0 release builds, Typst 0.15.1, and the **same 0.1.7
configuration/library files** with generated independent articles. No old CLI
patch, fake Typst, or omitted failed output is used for the comparison.

Linux, Intel Core Ultra 9 185H, 22 logical CPUs. Median of five runs per scenario,
cold Typsite cache per run, isolated child HOME/package directories, fixed
`SOURCE_DATE_EPOCH`, project/embedded fonts. OS filesystem caches are not flushed.
The older running demo preview was paused during earlier controlled measurements,
with permission, and resumed afterward. It had already exited before the final
run reported below; no replacement preview was running, and none was restarted.
Other desktop processes/thermal variation remain possible; small timing
differences should not be treated as guarantees.

Each article has headings, code, math, a footnote, and repeated paragraphs. The
script checks actual page counts, the template edit, and the article edit even
for the legacy CLI, which does not emit structured counts. **All generated site
hashes match byte-for-byte** between versions in every measured scenario;
internal ownership/lock metadata is excluded from site-content hashes.

| Articles | Scenario | 0.1.7 | Optimized 0.1.8 | Speedup |
| ---: | --- | ---: | ---: | ---: |
| 64 | Cold build | 0.639 s | 0.591 s | 1.08× |
| 64 | Warm no-op | 9.03 ms | 6.26 ms | 1.44× |
| 64 | Template edit, all pages rewritten | 0.153 s | 0.071 s | 2.16× |
| 64 | One article edit | 0.174 s | 0.117 s | 1.49× |
| 256 | Cold build | 2.194 s | 2.302 s | 0.95× |
| 256 | Warm no-op | 11.26 ms | 13.60 ms | 0.83× |
| 256 | Template edit, all pages rewritten | 0.221 s | 0.194 s | 1.14× |
| 256 | One article edit | 0.220 s | 0.209 s | 1.05× |

**The result is not faster in every scenario.** The final 64-page medians improve
in all four scenarios, and both sizes improve for template/article edits. But the
256-page cold build is about 5% slower and its no-op about 21% slower. Earlier
five-run controlled measurements had 64/256-page cold ratios of 0.95×/0.93× and
no-op ratios of 1.22×/1.08×; the small cold/no-op margins are not stable universal
wins. These regressions and variation are reported rather than selecting only
the most favorable run.

The old publisher wrote destination files directly without the new
atomic/sync/ownership protocol, and did not validate actual Typst dependency
generations. We retain those correctness/durability checks. No blanket “always
faster than 0.1.7” claim is justified, and larger cold/no-op builds remain an
explicit optimization target.

Before this follow-up, current `444f704d` on the same 64-page fixture took about
3.62 s cold, 85.6 ms no-op, 0.819 s template edit, and 0.217 s article edit. Those
numbers explain the targeted work, but are **not** the 0.1.7 baseline above.

## Reproduce

The dependency-free script always uses one chosen resource tree for both
binaries. To compare actual 0.1.7, create a separate jj workspace (or an existing
source checkout), leaving the current workspace and branches untouched:

```sh
jj workspace add /tmp/typsite-v017 -r 5038f5bb --no-colocate
env RUSTFLAGS='--cfg tokio_unstable' cargo build \
  --manifest-path /tmp/typsite-v017/Cargo.toml --release --locked \
  --target-dir /tmp/typsite-v017-target
cargo build --release --locked

python3 scripts/benchmark-build.py \
  --binary /tmp/typsite-v017-target/release/typsite \
  --resources /tmp/typsite-v017/resources --independent-only \
  --pages 64 --runs 5 --label v017 --output /tmp/v017.json
python3 scripts/benchmark-build.py --binary target/release/typsite \
  --resources /tmp/typsite-v017/resources --independent-only \
  --pages 64 --runs 5 --label optimized --output /tmp/optimized.json
```

Repeat with `--pages 256`. For the current full demo instead, omit `--resources`
and `--independent-only` (the demo now uses helpers absent from the old binary).
Use `--jobs` to vary export concurrency. Stop or pause competing workloads only
with their owner's permission. The script records individual timings, medians,
counts where available, and output hashes; compare hashes as well as speed.
Child-only HOME/environment/cwd changes do not alter the invoking process or
normal installed packages. Merely leaving a jj workspace does not delete it.
