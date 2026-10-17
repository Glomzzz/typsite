
# Typsite
[ **English** | [中文](./README-cn.md) ]

<div style="text-align: center;">
<img src="./icon.png" width="37.5%"/>
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
cargo test --test typst_latest --locked -- --ignored
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

## 5. Architecture & Flow

![alt text](./process.png)

Here's more: [Typsite Documant](https://typ.rowlib.com/en/)

## Contribution

We welcome your contributions to Typsite!

## Acknowledgements

- [kokic](https://github.com/kokic) : [kodama](https://github.com/kokic/kodama) for **Markdown + Typst + LaTeX SSG**
- [tola-rs](https://github.com/tola-rs) : [tola-ssg](https://github.com/tola-rs/tola-ssg) for **Typst-based SSG, focus on content**
