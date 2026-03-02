#import "/lib/lib.typ": *

#show: schema.with("page")


#title[Typsite Documentation]
#date[2025-04-12 02:08]
#author[Glomzzz]

#get-metacontent("icon", from: "/index.typ")

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

- Download the binary from the #link("[https://github.com/Glomzzz/typsite/releases/latest")[release]
  - Make sure you have *Typst 0.13+* installed
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
│   └── lib.typ      --- Typsite library file
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

#import "@preview/frame-it:1.2.0": *

#let (example, feature, variant, syntax) = frames(
  feature: ("Feature",),
  variant: ("Variant",),
  example: ("Example", gray),
  syntax: ("Syntax",),
)

#show: frame-style(styles.boxy)

Typsite is compatible with most native *Typst* syntax:

#get-metacontent("typst-support", from: "/index.typ")

#footnote[For `align`, please use the `html.align` function from the library] <align>
#footnote[For complex styled elements, use the `inline` function for *SVG* embedding]

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
      --no-pretty-url
      --no-short-slug
  -h, --help                 Print help
```

== clean

```
Clear cache and output directories

Usage: typsite clean [OPTIONS]

Options:
  -o, --output <OUTPUT>  Output directory [default: ./publish]
  -c, --cache <CACHE>    Cache directory for typst_html_export content [default: ./.cache]
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


= Architecture & Flow

#get-metacontent("process", from: "/index.typ")

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
