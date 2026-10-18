#import "/lib/lib.typ": *

#show: schema.with("page")


#title[Typsite Documentation]
#rss(true)
#date[2025-04-12 02:08]
#author[Glomzzz]

#get-metacontent("icon", from: "/index.typ")

#html.align(center)[ #cite-title("./migrate-to-018.typ") ]
#html.align(center)[ #cite-title("./migrate-to-017.typ") ]
#html.align(center)[ #cite-title("./migrate-to-016.typ") ]

= Introduction
*Typsite* is a tool for building static websites. It uses pure *Typst* to write content and processes it through *Typsite* to generate a fully functional static site.

= Features

- Standard *Typst* expressions
- Framework support: hierarchical headings, section templates, sidebar, footer
- Rich text: paragraphs, sections, quotes, code blocks, math formulas, footnotes, page embedding
- Typst math -> Mathml （auto detected math-font)
- Supports modern web standards such as HTML5, CSS3, and JavaScript (ES6+)
- Incremental compilation and real-time preview

= #embed("./typst.typ", sidebar: "only-title")

= Installation

- Download the binary from the #link("https://github.com/Glomzzz/typsite/releases/latest")[release page]
  - Use the *latest stable Typst* (currently tested with *0.15.1*)
  - Building from source requires *Rust 1.89 or newer*
- Build via Nix & Flakes
  - Ensure you’ve enabled `experimental-features = nix-command flakes`

```shell
git clone https://github.com/Glomzzz/typsite.git

cd typsite
nix build .
```

= Initialization

Run `typsite init` to initialize a Typsite project in the current directory.

```
.
├── root         ---  Typst root directory
│   ├── index.typ    --- Main article
│   └── lib/         --- Typsite library files; entry point: lib/lib.typ
├── .typsite     ---  Typsite configuration directory
│   ├── assets       ---  Resources (synced to output directory)
│   ├── components   ---  Component templates
│   ├── themes       ---  Code highlighting themes
│   ├── rewrite      ---  Rewrite templates
│   ├── schemas      ---  Page templates
│   └── options.toml ---  Project configuration
├── .cache       ---  Cache directory
└── publish      ---  Output directory
```

= Writing

Typsite is compatible with most native *Typst* syntax:

#get-metacontent("typst-support", from: "/index.typ")

#footnote[For `align`, please use the `html.align` function from the library] <align>
#footnote[For complex styled elements, use the `inline` function for *SVG* embedding] <svg-inline>

= CLI (Command Line Interface)

```
Usage: typsite <COMMAND>

Commands:
  init     Initialize a new Typsite project in the specified directory
  compile  Compile or watch a project; specify input/output [alias: c]
  clean    Clear cache and output directories
  syntect  Check the list of supported syntax and code highlighting
  help     Print this message or help info for a subcommand

Options:
  -h, --help     Print help
  -V, --version  Print version info
```

== init

```
Initialize a new Typsite project in the specified directory

Usage: typsite init [OPTIONS]

Options:
  -d, --dir <DIR>  Project root directory [default: ./]
  -h, --help       Print help
```

== compile

```
Compile or watch the project with specified input and output directories

Usage: typsite compile [OPTIONS]

Options:
      --host <HOST>          Serve host [default: localhost]
      --port <PORT>          Serve port, must be specified to watch mode [default: 0]
      --config <CONFIG>      Project config [default: ./.typsite]
      --cache <CACHE>        Cache dir [default: ./.cache]
  -j, --jobs <JOBS>          Concurrent article exports [default: automatic, CPU-aware, at most 4]
  -i, --input <INPUT>        Typst root dir, where your typst files are stored [default: ./root] [aliases: --i]
  -o, --output <OUTPUT>      Output dir [default: ./publish] [aliases: --o]
  -p, --packages <PACKAGES>  Packages dir, will be installed to @local and will be synced to @local in watch mode, skip if empty or not found [default: ] [aliases: --p]
  -t, --typst <TYPST>        Typst executable path [default: typst]
      --no-pretty-url
      --no-short-slug
  -h, --help                 Print help
```

== clean

```
Clear cache and output directories

Usage: typsite clean [OPTIONS]

Options:
      --input <INPUT>    Protected Typst source directory [default: ./root]
      --config <CONFIG>  Protected configuration directory [default: ./.typsite]
      --force            Allow cleaning unmarked legacy directories after path checks
  -o, --output <OUTPUT>  Output directory [default: ./publish]
  -c, --cache <CACHE>    Cache directory for raw Typst HTML [default: ./.cache]
  -h, --help             Print help
```

== syntect

```
View the list of syntax highlighting and supported languages
Usage: typsite syntect [OPTIONS]

Options:
      --config <CONFIG>  Config path [default: ./.typsite]
  -h, --help             Print help
```


== Build parallelism

Typsite uses Rayon to hash independent files, check export caches, parse article HTML, and render pages in parallel. Typst exports run in a separate bounded Rayon pool: the automatic limit is the available CPU budget (also respecting the Rayon pool size), capped at four simultaneous articles. Each Typst subprocess receives a share of that CPU budget through its own `--jobs` option, rather than letting every subprocess spawn a full machine's worth of workers.

```sh
typsite compile --jobs 4
# Low-memory or export debugging: one article subprocess at a time.
typsite compile --jobs 1
```

`-j` is an alias for `--jobs`; the value must be positive. This setting controls simultaneous *article exports*, not all Rayon work. Use `RAYON_NUM_THREADS` to tune Rayon hashing/parsing/rendering separately. An explicit high `--jobs` limit can increase memory use; it is still bounded by the number of dirty articles. No export pool is created for true no-ops.

Packages are prepared before exports start. Workers produce independent artifacts, then the build joins them and records results in stable source order. Graph resolution, dependency-ordered metadata, cache checkpoints, and ownership/publication mutations stay ordered for correctness. Changing `--jobs` alone does not invalidate the cache or change site bytes.

= RSS feed and sitemap

*RSS is opt-in.* The article flag `rss` defaults to `false`, and the bundled `base_url` is empty. To publish a feed, configure the site URL *and* select articles with `#rss(true)` on a collection page. Do not set the global article default to true.

== 1. Enable feed generation

Set the public site URL in `.typsite/options.toml` to generate RSS 2.0 and a sitemap:

```toml
[site]
base_url = "https://example.org"
title = "My site"
description = "Notes and articles"
rss_path = "/rss.xml"
sitemap_path = "/sitemap.xml"
rss_limit = 20
rss_content = "summary" # "summary" or "full"
```

Do not add a second `[site]` section; edit the existing one. Replace the example URL with your actual public URL, then run `typsite compile`. The files are written beneath the output directory. Every generated page receives an RSS autodiscovery link in its HTML head; readers can subscribe to `https://example.org/rss.xml`.

== 2. Select articles with a container

Article metadata has a boolean `rss` option, defaulting to `false`. Mark a collection page with `#rss(true)`, then directly embed the articles to publish:

```typ
#import "/lib/lib.typ": *
#show: schema.with("page")
#title[My feed collection]
#rss(true)

= #embed("./first-post.typ")
= #embed("./second-post.typ")
```

Only those *directly embedded articles* become feed items. The marked collection page itself is excluded, even if another collection embeds it. The marker is not inherited by embedded articles. Nested embeds are not selected as separate entries; their content still appears inside a selected article in full mode. Citations, parent metadata, and graph-only child declarations do not select items. Multiple containers contribute a union of entries, with each article appearing only once.

`#rss(false)` makes a page an ordinary article again; it can still be included if a marked container embeds it. With no marked containers (or no eligible direct embeds), the enabled feed is empty. Date ordering and `rss_limit` apply *after* selection. The sitemap still lists all compiled articles.

Both demo homepages are marked containers. They directly embed the article guide and Typst introduction in each language, so the demo feed has *four entries*, not every page. For older customized libraries, import `rss` from `@local/typsite:0.1.0` or merge the new `root/lib/lib.typ` import.

== 3. Try it in this demo

After setting `base_url`, run these commands from the initialized demo directory:

```sh
typsite compile
typsite compile --port 8080
```

Inspect `publish/rss.xml`, or open `http://localhost:8080/rss.xml` while preview is running. Deploy the output and subscribe using your public URL, such as `https://example.org/rss.xml`; the preview address is only for local checks.

No file? Check `base_url`, `rss_path`, and `rss_limit`. Empty feed? Check for a marked container with eligible direct embeds. Keep `[default_metadata.options].rss = false`; `#rss(true)` belongs on the collection, not on the posts you want to publish. A self-contained upgrade walkthrough is in #cite("./migrate-to-018.typ")[the 0.1.8 RSS migration instructions].

== Item content and dates

- `summary` includes a plain-text description, taking the first nonempty `description`, `summary`, or `abstract` metadata value. It does not invent an excerpt when these are absent.
- `full` additionally includes the composed article HTML in `content:encoded`, including embedded articles but not the page schema, navigation, sidebar, or reference/backlink footer. Ordinary links and image URLs become absolute. Scripts are omitted; feed readers may not display SVG, MathML, interactive elements, or site-specific CSS exactly like the website.
- Titles use `page-title`, then `title`. Articles are ordered by `updated`, `lastmod`, then `date`, newest first, with URLs breaking ties. `pubDate` uses the publication `date`, falling back to the modification time.
- Dates accept `YYYY-MM-DD`, `YYYY-MM-DD HH:MM[:SS]` (UTC), RFC 3339, or RFC 2822. Invalid or unknown dates are omitted rather than emitting invalid RSS dates.
- An empty `base_url` disables both outputs. An empty `rss_path` or `rss_limit = 0` disables RSS only. An empty `sitemap_path` disables only the sitemap. Disabled or renamed generated files are cleaned up through the ownership journal.
- Changing feed settings only recomposes outputs; unchanged articles are not re-exported with Typst. `--no-pretty-url` is reflected in article links.

Add metadata to an article with the existing helpers:

```typ
#title[An article]
#date[2026-03-02 18:25]
#metacontent("summary")[A short introduction for feed readers.]
#metacontent("updated")[2026-03-03T09:00:00Z]
```

= Architecture & Flow

#get-metacontent("process", from: "/index.typ")

Follow the square diagram's numbered snake: left to right across the first row, right to left across the second, then left to right across the third. HIT reuses raw HTML; MISS invokes Typst. An unchanged published site exits at step 03; only dirty publication continues through parsing, graph resolution, and whole-page composition. Independent checkpoints preserve exports and publication state, and the bottom red band explains recovery. On small screens, scroll within the diagram to read the details.

Rayon parallelizes independent hashes, article parsing, page rendering, and bounded Typst exports; package setup and publication checkpoints remain ordered.
The cache stores raw Typst HTML and actual dependency hashes, not old relationships or rendered fragments.
Dirty builds recompose all pages but write only changed bytes; true no-ops neither write output nor reload preview clients.
Publication is atomic per file, not for the whole site; fix a reported failure and rerun to recover.
See #cite-title("./migrate-to-018.typ") for migration and output-ownership details.

= Configuration

You can view all default configurations here:

Based on these, you can fully customize your entire site.

- *schema*: Page templates, handling page structure
- *components / rewrites*: Components/Rewriters, build up the page content
- *assets*: Resource files, automatically synced during compilation
- *themes*: Code highlighting files
- *syntaxes*: Code syntaxes files

= #embed("./article.typ", sidebar: "only_title", open: false)
#html.align(center)[Why not take a look at #cite-title("./example.typ") first?]
