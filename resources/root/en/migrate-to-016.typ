#import "/lib/lib.typ": *

#show : schema.with("page")

#let version = html.text(fill: rgb("#22D3EE"))[0.1.6]

#title[Migrating to Typsite #version]
#page-title[Migrating to Typsite 0.1.6]
#date[2025-06-21 02:47]
#author[Glomzzz]
#parent("index.typ")

#let breaking-changes = html.text(fill: red.darken(10%))[breaking-changes]

In version #version, Typsite introduced a series of major updates, inevitably including some #breaking-changes. This document aims to help users migrate to the new version smoothly.

= Typsite Packages

Parts of the Typst library that are closely related to Typsite have been extracted into separate Typst packages. You can find them under #link("https://github.com/Glomzzz/typsite/tree/main/packages/")[typsite/packages]. Typsite bundles these packages into the binary and automatically installs them to the `@local` directory during each `typsite compile`.

To facilitate debugging and using packages, Typsite also supports local package auto-installation. Just specify the `--packages` flag when running `typsite compile`.

- In `watch-mode`, Typsite will also automatically update these local packages and recompile the site.

So now, the Typsite Typst libraries are split into two parts:
- `@local/typsite:0.1.0`: Contains core functionality of Typsite, provides basic functions, and acts as an interface between Typsite and site libraries.
    - Thanks to this layer, it is unlikely for future updates to introduce more #breaking-changes.
- `root/lib/`: The site library of each Typsite site. Users are free to customize anything here.

= Migration

If you're relying on site libraries from before version #version, you'll need to make the following replacements when migrating:

- `import "lib.typ"` |-> `import "/lib/lib.typ"`
- `html-text` |-> `html.text`
- `text-align` |-> `html.align`
  - (Make sure *not*- to replace `text-align` inside `/lib/site.typ`)

It's recommended to import `lib.typ` using an absolute path. If you're using #link("https://github.com/Myriad-Dreamin/tinymist")[tinymist] as your LSP, you'll need to specify the Typst `root-path` for better experience:

- In VSCode, you can add the following to `.vscode/settings.json` in your site directory:
  ```json
  "tinymist.rootPath": "absolute path of your site/root"
  ```

For any #breaking-changes not anticipated here, you can easily detect them by launching `watch-mode` with `typsite compile --port 8000`. Typsite will provide helpful error messages whenever you modify a file, allowing you to fix issues quickly.

= Other Updates

- Added support for converting Typst math to MathML.
  - This improves the rendering of math formulas, and the MathML conversion will detect Typst math fonts in-place for compatibility.
- `inline` function has a new `fit-font` parameter, which can adapt to Typst's `context text.size`.
- For pages containing backlinks and references, duplicate tags in the `<head>` section are now removed, resulting in a much cleaner structure.
- In the default site library, `show-rules` have been modularized. You can easily add/remove/modify `show-rule` items in `/lib/lib.typ`.
- In the default site configuration, link titles have been optimized. You can now preview links as `Link Title [Link URL]`.
