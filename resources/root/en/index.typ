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


= RSS feed and sitemap

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

== Select articles with a container

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
