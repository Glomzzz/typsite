#import "/lib/lib.typ": *

#show : schema.with("page")

#let version = html.text(fill: rgb("#22D3EE"))[0.1.7]

#title[Migrating to Typsite #version]
#page-title[Migrating to Typsite 0.1.7]
#date[2026-03-02 18:25]
#author[Glomzzz]
#parent("index.typ")

#let breaking-changes = html.text(fill: red.darken(10%))[breaking-changes]

In version #version, Typsite introduced a series of major updates about SVG-compat, inevitably including some #breaking-changes. This document aims to help users migrate to the new version smoothly.

= 0.1.6 Migration

Make sure you've done： #cite-title("migrate-to-016.typ")


= .typsite Configuration Migration

In version #version, the following new components and rewrites were added to the .typsite directory:

- `.typsite/components/anchor_def_svg.html`: SVG anchor definition component
- `.typsite/components/anchor_goto_svg.html`: SVG anchor goto component
- `.typsite/rewrite/footnote-ref-svg.html`: Footnote reference rewrite for SVG

If you're upgrading from an older version, make sure to copy these new files to your site's .typsite directory.

= /root/lib Library Migration

== inline Function Signature Change

The `fit-font` parameter semantics of the `inline` function have changed:

- Before #version: `fit-font` was a `bool` type
- After #version: `fit-font` is a `1em` type (can be `true`, `false`, or values like `1em`, `2em`)

Migration:
- `inline(content, fit-font: true)` |-> `inline(content, fit-font: 1em)`
- `inline(content, fit-font: false)` |-> `inline(content)` or `inline(content, fit-font: false)`

== Library Import Changes

If you've customized the `inline` function in your site library, you need to update the call signature:

```typ
// Old version
#import "@local/typsite:0.1.0": inline

// New version
#let footnotes = state("footnotes", ())
#let inline(..args) = context {
  import "@local/typsite:0.1.0": inline
  inline(..args, footnotes: footnotes.get())
}
```

== rule Function Signature Changes

The following functions in `rule.typ` now need to receive the `footnotes` parameter:

- `rule-ref(footnotes)` instead of the old `rule-ref`
- `rule-footnote(footnotes)` instead of the old `rule-footnote`

In the `schema` function, you need to fetch footnotes first and then pass them to the rules:

```typ
let _footnotes = query(footnote).map(it => it.at("label", default: none)).filter(it => it != none)
show: rule-footnote(_footnotes)
show: rule-ref(_footnotes)
```

= /packages Package Migration

== inline Function New footnotes Parameter

The `inline` function in `@local/typsite:0.1.0` now requires a `footnotes` parameter:

```typ
// Old version
inline(content, fit-font: true, scale: 100%)

// New version
inline(content, fit-font: 1em, scale: 100%, footnotes: ())
```

== auto-sized-svg New fit-font Parameter

The `auto-sized-svg` function now has a `fit-font` parameter with more flexible font size control:

- `true`: Use CSS font-size inheritance
- `false`: Use fixed font-size: 1rem
- `1em`: Use relative em value

= Zed Editor Compatibility

== Zed LSP Configuration

If you're using #link("https://zed.dev")[Zed] as your editor, you can add the following configuration for a better Typst development experience:

Add to `.zed/settings.json`:

```json
{
  "lsp": {
    "tinymist": {
      "initialization_options": {
        "exportPdf": "never",
        "rootPath": "your/site/root/path",
      },
    },
  },
}
```

== VSCode Settings Migration

If you were previously using VSCode with tinymist configured, you need to update `rootPath` to an absolute path:

In `.vscode/settings.json`:

```json
{
  "tinymist.rootPath": "/absolute/path/to/your/site/root"
}
```

= 0.1.6 Migration Link

If you're migrating from version #link("migrate-to-016.typ")[0.1.6], make sure you've also completed the 0.1.6 migration steps:

- `import "lib.typ"` |-> `import "/lib/lib.typ"`
- `html-text` |-> `html.text`
- `text-align` |-> `html.align`

= Other Updates

- Added SVG anchor support, allowing creation of jumpable anchors in Typst content
- Footnote references now support SVG inline rendering
- Improved the font size adaptation mechanism for the `inline` function
- Added support for more flexible `fit-font` parameter, allowing specific em values
