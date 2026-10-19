
# Typsite
[ **English** | [中文](./README-cn.md) ]

<div style="text-align: center;">
<img src="./resources/root/icon.svg" width="37.5%"/>
</div>
<div style="text-align: center;">
<a href="resources/root/en/migrate-to-018.typ" title="Migrating to Typsite 0.1.8">Migrating to Typsite <span style="color: #22d3ee;">0.1.8</span></a>
</div>
<div style="text-align: center;">
<a href="https://typ.rowlib.com/en/migrate-to-017" title="Migrating to Typsite 0.1.7">Migrating to Typsite <span style="color: #22d3ee;">0.1.7</span></a>
</div>
<div style="text-align: center;">
<a href="https://typ.rowlib.com/en/migrate-to-016" title="Migrating to Typsite 0.1.6 [https://typ.rowlib.com/en/migrate-to-016]">Migrating to Typsite <span style="color: #22d3ee;">0.1.6</span></a>
</div>

## 1. Introduction

Typsite is a static site generator (SSG) that uses pure `Typst` for content creation. It processes these `Typst` files to generate a complete static website.

For a real-world example, see [glomzzz.com](https://glomzzz.com/) ([source code](https://github.com/Glomzzz/OOM)). The site is in Chinese.

## 2. Features

-   Core `Typst` expressions and syntax
-   Framework: Incremental headings, section templates, sidebar, footer
-   Rich Text: Paragraphs, sections, quotes, code blocks, math formulas, footnotes, page embeds
-   Typst math -> Mathml （auto detected math-font)
-   Support for modern web technologies: HTML5, CSS3, and JavaScript (ES6+)
-   Automatically typst-packages install & sync (in watch-mode)
-   Links, Footnotes in SVG (0.1.7+)
-   Incremental compilation and real-time preview
-   RSS 2.0 feeds with configurable summaries/full articles, plus sitemap output
-   Native PDF books from an entry article's embeds, with ordered appendices


## 3. About Typst

Typst is a modern typesetting system, similar to LaTeX but designed to be simpler and easier to learn. It's primarily used for creating beautifully typeset documents like academic papers, books, and reports.

You can find the official English documentation here: [Typst Document](https://typst.app/docs/)
For a Chinese tutorial on Typst, I highly recommend the [Typst Blue Book](https://typst-doc-cn.github.io/tutorial/introduction.html) (The author has a remarkably clear understanding of the subject!).

The rest of this section explains the connection between Typst, HTML, and Typsite.

### 3.1 Typst's HTML Export Feature

Typst introduced HTML export functionality in version `0.13`. This includes an `html-export` mode and two core functions: `html.elem` and `html.frame`. These allow us to write content in Typst that targets HTML+CSS output.

### 3.2 Typsite: A Typst-based Static Site Generator

Inspired by this, I developed a static site generator named `Typsite` using `Rust`.

Currently, Typst's HTML export:
-   Has good support for simple rich text.
-   Complex styling requires users to manually write it using the `html.elem` function.
-   Cannot automatically convert all Typst ecosystem content to HTML.
-   For content with complex Typst styling, `html.frame` can be used to convert it to SVG and embed it in HTML.
-   Each compilation only supports single-file HTML output.

For details on supported features and plans, you can track this [issue: HTML export #5512](https://github.com/typst/typst/issues/5512).

While there are official plans for Typst to automatically convert styles to HTML+CSS, this doesn't conflict with Typsite. In fact, Typst's advancement will make Typsite even more useful, as Typsite's primary role is to manage inter-article interactions and build a fully-featured static website.

## 4. Installation

-   Start with template: [Typsite Template](https://github.com/Glomzzz/typsite-template) (HIGHLY RECOMMENDED, for linux / macos)
-   Download the binary from the [Release page](https://github.com/Glomzzz/typsite/releases/latest).
    -   Use the **latest stable Typst** (currently tested with **0.15.1**). CI checks both 0.15.1 and the latest stable release.
-   Build using Nix & Flakes.
    -   Ensure you have enabled `experimental-features = nix-command flakes` in your Nix configuration.

```shell
git clone https://github.com/Glomzzz/typsite.git
cd typsite
nix build .
```

### 4.1 Building and checking from source

Use stable Rust **1.89 or newer** and the latest stable Typst:

```sh
cargo build --release --locked
cargo test --workspace --all-targets --locked
# Real Typst tests (isolated packages, helpers, templates, and the entire demo):
cargo test --test typst_latest --test typst_pdf --locked -- --ignored
```

Existing authoring helpers, templates, and CLI options remain supported. The
build engine now reports export/render/publication failures instead of leaving
stale output with a successful exit status. Builds track actual Typst imports;
dirty builds recompose whole pages but write only changed bytes.

Cache formats are disposable. Output ownership is tracked separately in
`.typsite-outputs.json`; keep this file to allow safe cleanup of deleted pages.
`clean` protects project source/configuration paths. For unmarked legacy
cache/output directories, use `clean --force` only after checking the paths;
pass `--input` / `--config` when those protected directories are nondefault.
See the [0.1.8 migration guide](resources/root/en/migrate-to-018.typ) and
[build architecture, validation changes, and limitations](docs/refactor.md).

### 4.2 Run the bundled demo

`init` copies the complete bilingual demo, including migration guides, into a new
directory. No preview-package downloads are required to build it:

```sh
typsite init --dir ./demo
cd demo
typsite compile
typsite compile --port 8080
```

Open `http://localhost:8080/` or `http://localhost:8080/en/`. To build the demo
straight from the repository without initializing another project:

```sh
cargo run --locked -- compile --input ./resources/root \
  --config ./resources/.typsite --cache ./target/demo-cache \
  --output ./target/demo-site
```

Keep `publish/.typsite-outputs.json` (or the journal in your custom output root).
External music players and web fonts in the examples need browser network access,
but are not build dependencies.

### 4.3 RSS subscriptions

Edit the existing `[site]` section in `.typsite/options.toml`:

```toml
[site]
base_url = "https://example.org"
title = "My site"
description = "Notes and articles"
rss_path = "/rss.xml"
rss_limit = 20
rss_content = "summary" # "summary" (default) or "full"
sitemap_path = "/sitemap.xml"
```

Select items by marking a collection article with the boolean `rss` metadata:

```typ
#import "/lib/lib.typ": *
#show: schema.with("page")
#title[Feed collection]
#rss(true)
= #embed("./first-post.typ")
= #embed("./second-post.typ")
```

Only **direct embeds of `rss: true` pages** enter the feed. Marked containers
are always excluded; nested embeds, citations, and parent relationships do not
select separate items. Multiple containers are deduplicated. The flag defaults
to false and is not inherited. With no containers, the feed has no entries.
`#rss(false)` removes a container marker, not an article selected by another
container. Sitemap generation still covers all compiled articles.

Run `typsite compile`: `publish/rss.xml` is ready for readers to subscribe to,
and generated pages include an RSS autodiscovery link. Summary mode uses the
first nonempty `description`, `summary`, or `abstract` metadata. Full mode also
includes composed article HTML in `content:encoded`, without site navigation or
footers. Links/images are made absolute; scripts are omitted. Reader support for
SVG, MathML, and site-specific CSS varies.

An empty `base_url` disables both feeds and sitemap. Set `rss_path = ""` or
`rss_limit = 0` to disable RSS alone. Entries sort by modification/publication
metadata, not the current build time, so no-op builds do not change the feed.
See the [Typst documentation](resources/root/en/index.typ) for date formats,
article metadata examples, and all settings.

### Build parallelism

Typsite uses **Rayon** for independent file hashing, cache checks, article parsing,
and page rendering. Typst article exports use a separate bounded Rayon pool:
CPU-aware by default (up to the available CPU/Rayon budget), with the budget
divided between Typst subprocesses rather than letting each use all cores.

```sh
typsite compile --jobs 4  # also: -j 4
typsite compile --jobs 1  # sequential article export; lower peak memory
```

`--jobs` must be positive and controls article exports, not all Rayon work.
`RAYON_NUM_THREADS` tunes hashing/parsing/rendering separately and influences the
automatic export budget. Explicitly large job limits may increase memory use.
Packages are prepared once before export workers start; results and cache updates
are folded in source order. Graph/metadata dependencies, stale deletions, and
ownership checkpoints stay ordered; independent output files publish in parallel
with per-file atomic renames, data syncing, and batched parent-directory syncing.
Verified no-ops skip configuration parsing. Themes/syntaxes have a content-keyed
cache, and shared Typst dependencies are verified once per export batch. Release
builds optimize for speed (`opt-level = 3`). Changing jobs alone does not invalidate
cached output.

See [measured before/after results and the reproducible benchmark](docs/performance.md).

### 4.4 Migrating to 0.1.8

Follow the [English migration guide](resources/root/en/migrate-to-018.typ)
([中文](resources/root/migrate-to-018.typ)), bundled as a Typst page in the demo.
The upgrade checklist is:

1. Back up your articles, `root/lib`, and `.typsite`. Use the latest stable Typst
   (tested with **0.15.1**); source builds need **Rust 1.89+**.
2. Generate a reference project with `typsite init --dir ../typsite-018-reference`.
   Compare and merge its helpers/configuration; do not initialize over your site.
   Update `root/lib/site.typ` for math fallback and import `rss` in
   `root/lib/lib.typ` if you want feed collections. The bundled Typst package
   keeps the **`@local/typsite:0.1.0`** import identity.
3. Keep `.typsite-outputs.json` in the output directory. Old internal caches
   rebuild automatically; `clean --force` is only needed for explicitly cleaning
   unmarked legacy directories after checking their paths.
4. For RSS, set `[site].base_url` and mark collection pages with `#rss(true)`;
   **`rss` defaults to false**. Only their direct embeds become items, not the
   containers themselves. Choose `rss_content = "summary"` or `"full"`.
5. Run `typsite compile` and check its exit status before deploying. Malformed
   resources and recursive embed/parent/metadata graphs now fail instead of
   being silently skipped or leaving a falsely successful build.

See the migration guide for copyable configuration, safe cleanup, package
isolation, RSS troubleshooting, and failure recovery. See [stable release gates](docs/releasing.md)
for exact-revision CI, locked builds, downloaded-binary smoke tests, and manual
draft publication; local Linux tests alone do not establish cross-platform readiness.

### 4.5 Native PDF books

```sh
typsite pdf --entry index.typ --output book/site.pdf
# Optional standalone appendices and an editable native Typst style:
typsite pdf --appendix appendices/notation.typ --template lib/pdf.typ
typsite pdf --front-cover lib/front-cover.typ --back-cover lib/back-cover.typ
```

Existing embed calls, including `= #embed("./child.typ")`, expand in source order.
Repeated embeds print again; citations do not automatically include their targets.
For outside-book citations, set `site.base_url` or include the target as an appendix.
The result is one native Typst document with math, footnotes, links, and an outline,
not an HTML printout. `--output` names a **PDF file**. HTML outputs and their
ownership journal are untouched; the default PDF output is `book/site.pdf`.

Existing sites should merge the updated authoring helpers from a reference
`typsite init` project. Native Typst labels remain document-global, so repeated
articles with numbered native references need unique labels or occurrence-local
Typsite anchors. See [PDF setup, semantics, styling, safety, and limitations](docs/pdf.md).

## 5. Architecture & Flow

[![Typsite build pipeline: three lanes (inputs and cache, build, publish), nine numbered steps, a no-op exit, two checkpoints, and failure recovery](./process.png)](./process.png)

Each lane reads left to right, top to bottom. Steps 01–03 hash inputs and reuse
or re-run Typst exports (HIT/MISS); if the published output is already current,
the build stops at the no-op exit. Otherwise steps 04–06 parse articles, resolve
the relationship graph, and compose pages and feeds, then steps 07–09 validate
the output plan, publish only changed bytes, and report. Tags mark Rayon-parallel
stages and the two checkpoints (A: export cache, B: ownership journal); the red
band shows how failures before and during publication recover. Click the image
for the full-size diagram.

This preview and the demo share the same
[Typst diagram source](resources/root/lib/process.typ). Regenerate the preview
from the repository root with the bundled font:

```sh
typst compile --root . --font-path resources/.typsite/assets/fonts \
  --ignore-system-fonts --ppi 144 docs/process.typ process.png
```

[Build guarantees and limitations](docs/refactor.md) ·
[Demo documentation source](resources/root/en/index.typ) ·
[Published documentation](https://typ.rowlib.com/en/)

## Contribution

We welcome your contributions to Typsite!

## Acknowledgements

- [kokic](https://github.com/kokic) : [kodama](https://github.com/kokic/kodama) for **Markdown + Typst + LaTeX SSG**
- [tola-rs](https://github.com/tola-rs) : [tola-ssg](https://github.com/tola-rs/tola-ssg) for **Typst-based SSG, focus on content**
