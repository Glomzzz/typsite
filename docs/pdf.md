# Native PDF books

`typsite pdf` compiles an entry article and its embedded articles into **one native
Typst document**. It does not print the website, convert HTML, or merge independent
PDFs. Native math, images, footnotes, pagination, links, and the document outline
remain available.

Use the latest stable Typst (tested with 0.15.1). Existing projects need the
updated bundled authoring helpers; see [upgrading helpers](#upgrading-helpers).
The package import identity remains `@local/typsite:0.1.0`.

## Commands

From an initialized project:

```sh
typsite pdf
# Defaults: --input ./root --config ./.typsite --cache ./.cache
#           --entry index.typ --output ./book/site.pdf

typsite pdf --entry index.typ \
  --appendix appendices/notation.typ \
  --appendix appendices/references.typ \
  --template lib/pdf.typ --title "Collected notes" --output book/notes.pdf
```

`--output` is a **file**, unlike `compile --output`, which is a directory.
`--entry`, each repeatable `--appendix`, and `--template` are paths within
`--input`. Article references accept `.typ`, `.html`, and canonical slugs;
source-relative `.typ` paths are the least ambiguous choice for filenames
containing dots. A leading `/` means the source root, not the filesystem root.
Traversal outside that root is rejected.

`--packages`, `--typst`, and positive `--jobs`/`-j` are supported. Jobs controls the
worker budget of the native Typst process, not concurrent per-page exports. There
is no PDF watch/serve mode. The normal `compile` command continues to build HTML.

The bundled demo cites standalone guides that are not embedded in its index.
Include those targets as below (or configure `site.base_url` for external links):

```sh
typsite pdf --appendix migrate-to-016.typ --appendix migrate-to-017.typ \
  --appendix migrate-to-018.typ --appendix example.typ --template lib/pdf.typ
```

Chinese/emoji content needs suitable local fonts; configuring a font family does
not install that font. The bundled Inria Sans font is not a full CJK font.
Web font CSS does not install PDF fonts.

## Remote Chinese fonts (no installation or bundling)

Add `--font-profile noto-serif-sc` to download **Noto Serif SC 2.003** Regular
and Medium from the official `notofonts/noto-cjk` repository. Both files are
pinned to commit `9b0f1436e455d902de067a2501422e5dc71ad16b` and SHA-256 verified.
The SIL OFL 1.1 license is fetched and verified alongside them.

```sh
typsite pdf --template lib/pdf.typ --font-profile noto-serif-sc
# Subsequent builds can run without network access:
typsite pdf --template lib/pdf.typ --font-profile noto-serif-sc --offline
```

The files live only in `.cache/pdf/fonts/noto-serif-sc-<revision>/` (about 23 MB),
not in your source/configuration, executable, published HTML assets, or system
font directories. `--font-profile none` is the default and never downloads fonts.
HTTPS, certificate validation, time/size limits, checksum verification, and
atomic cache replacement protect downloads. A verified warm cache does not
contact the server or rewrite files. Corrupt caches are repaired online;
`--offline` reports missing/corrupt resources without touching the last PDF.
`clean` removes downloaded font caches, so offline builds need them prewarmed.

The driver selects the family, and the sample print template receives it as
`sys.inputs.at("typsite-font-family", default: "")`. Keep it in custom font lists
rather than overriding the entire list with a Latin-only font. The sample uses
10pt body text, medium-weight 15/12.5/11pt headings, and 500-weight strong text
instead of oversized, artificially heavy CJK fallback headings. All sizes and
weights are editable in `root/lib/pdf.typ`. HTML still uses its own CSS/web-font
setup; the remote profile applies only to PDF. Typst needs a local file while
compiling, but the download/cache is managed automatically—no manual install.

## Selection and ordering

- The entry is printed first. Embeds expand **in place**, preserving surrounding
  text, Typst-generated content, and source order.
- Both `#embed("./child.typ")` and `= #embed("./child.typ")` remain supported.
  Relative references belong to the owning article, not the library helper.
- Every embed occurrence prints the body again. A source embedded under two
  parents has two occurrence identities. It is not globally deduplicated.
- `open: false` still prints the body. Sidebar/folding options and HTML templates
  do not decide which content enters the PDF.
- Appendices follow in command-line order, including explicitly repeated roots.
  They use top-level letter numbering. Each can contain its own embed forest.
- Normal discovery honors `[typst_lib]`. **Explicit entry/appendix roots** can
  select standalone `.typ` pages in excluded directories, e.g.
  `--appendix lib/print-only.typ`, without adding them to the HTML site.
- Citations and metadata-parent declarations do not include article bodies.
- Missing embed targets and recursive embed stacks are errors. Repeated siblings
  and ordinary citation cycles are legal. Unselected sources are not compiled.

The book adds an article-title heading above each article. Local headings are
normalized beneath it in source order: a first `==` becomes the first local
level, and skipped levels descend by one structural level rather than inserting
zero counter components. Thus an article at `1.3` with `==` subsections prints
`1.3.1` and `1.3.2`, not `1.3.0.1`. Heading embeds participate in the same local
hierarchy. Plain embeds support `heading-level: "child"`, `"peer"`, or a positive
numeric local level. HTML heading structure is unchanged.

## Metadata and links

`title` and `metacontent` store invisible native content, not HTML strings.
`get-metacontent` resolves rich content in the appropriate occurrence:

- `$self` selects the current occurrence.
- `parent` uses an explicit `parent(...)` declaration if present; otherwise it
  uses the containing embed occurrence, then the configured default parent.
- An explicit article path selects its first printed occurrence, except a
  self-reference stays in the current occurrence.
- Missing keys inherit through that parent chain, then use
  `[default_metadata.content]` as **plain native text**. HTML in those defaults
  is not converted into Typst formatting.
- Recursive metadata lookup fails with a diagnostic. A referenced metadata
  source must be in the book; add it as an appendix if necessary.

A `cite` or `cite-title` to an included article links to its first printed
occurrence; self-citations stay local. Citation anchors and `goto` use that
occurrence's anchor index. Use `anchor`/`goto` from the package for explicitly
occurrence-local anchors:

```typ
#import "@local/typsite:0.1.0": anchor, goto
#anchor("proof")
#goto("proof")[Return to this occurrence's proof]
```

For a known but unselected article, citations use `[site].base_url` and the site
URL policy (`--no-pretty-url` retains `.html`). A title-only citation uses the
canonical slug as a visible fallback because that article's rich metadata was
not evaluated. Without `base_url`, an excluded destination is an error: include
it as an appendix or configure the website URL. A citation never silently causes
its target to be appended or disappears from the text.

**Native Typst labels remain document-global.** Repeating an article containing
`<equation>` and `@equation` can produce duplicate-label errors. The compiler
reports this limitation rather than rewriting arbitrary Typst labels. Native
numbered references (including custom supplements) work normally when labels
are unique. Typsite-managed anchors, explicit-supplement refs to unnumbered
content, and label links have occurrence-local indexes; these do not turn every
arbitrary native label operation into a scoped one.
The internal label `typsite-pdf-title` is reserved for article headings.

## Customizable link styles

The sample PDF template applies `style-links` from **`root/lib/links.typ`** to
all native links: article citations, anchors/`goto`, native references, and URLs.
Defaults match the website: **dotted underlines for local links**, **solid
underlines for external links**, with subdued text colors. HTTP(S), `mailto:`,
protocol-relative URLs, and other URI schemes count as external; document
locations, labels, and relative/root paths count as local. An outside-book
citation that resolves to a website URL therefore gets the external style.
Destinations remain real clickable Typst links, not painted text substitutes.

Edit `local-color`, `external-color`, `local-stroke`, `external-stroke`, or the
`local-style` / `external-style` functions in `links.typ`. The same module supplies
HTML's `.link.local` / `.link.external` wrappers; browser colors, hover treatment,
and underline styles remain customizable in `.typsite/assets/schemas/page.css`.
Custom print templates can `#import "links.typ": style-links` and
`show: style-links`, or replace that show rule with their own. Without the sample
print template, native Typst's default presentation remains available.

## Front and back covers (PDF only)

Use `--front-cover lib/front-cover.typ` and `--back-cover lib/back-cover.typ` to
add custom **front and back covers**. The aliases `--front-page` and `--back-page`
are also accepted. Editable examples ship in `root/lib/`, excluded from normal
HTML article discovery. Neither cover is included unless requested.

Covers are plain native Typst content, not articles: do not apply `schema` or use
article metadata/embeds there. They can use local images, rich text, links, and
normal layout functions. Paths resolve from the original cover source file.
Each cover gets its own page without running headers, footers, or printed page
numbers. A cover that overflows one page is an error; the last good PDF remains
untouched. Cover headings are excluded from the book's numbering and outline.
The front cover precedes the contents; the back cover follows every appendix.
Visible page numbering starts at 1 after the front cover. Cover options and
contents participate in normal cache invalidation and clean-build equivalence.

```sh
typsite pdf --template lib/pdf.typ \
  --front-cover lib/front-cover.typ --back-cover lib/back-cover.typ
```

## Footnotes

In PDF, a labeled definition such as `#footnote[Note text]<note>` is invisible
where declared. References (`#footnote(<note>)`, `@note`, or the footnote rewrite
helpers) create the note on the page where it is used. Repeated references on
one page share a marker and entry; a reference on another page creates that
page's own entry. Numbering restarts at 1 on each physical PDF page, not each
article. Anonymous `#footnote[Text]` remains an inline note.

Named notes are scoped to an article occurrence, including repeated embeds.
Keep named definitions outside opaque layout `context` expressions so they can
be collected before layout; declarations there fail rather than emit phantom
markers or entries. The internal label prefix `typsite-pdf-note-` is reserved.
HTML footnotes retain their existing behavior.

## Shared HTML/PDF headers and footers

Enable one editable **Typst module** in `.typsite/options.toml`:

```toml
[layout]
module = "lib/layout.typ"
```

Paths are relative to `--input` (a leading `/` means that source root). The
module must exist there and export both functions below. An absent `[layout]`
section or `module = ""` disables it; existing sites retain their old layout.
The selected module is excluded from implicit article discovery. `typsite init`
ships an editable `root/lib/layout.typ` example. Keep it independent of
`lib.typ`/`schema` to avoid a circular import.

```typ
#let page-header(info) = {
  link("https://example.org")[My collected notes]
}
#let page-footer(info) = {
  if info.target == "pdf" {
    align(center)[#info.title #h(1em) #info.page]
  } else {
    [Copyright 2026. #link("https://example.org/about")[About]]
  }
}
```

Both functions receive a dictionary:

| Field | HTML | PDF |
| --- | --- | --- |
| `target` | `"html"` | `"pdf"` |
| `title` | Rich standalone article title | PDF document title (`--title`/`site.title`/entry slug) |
| `source` | Canonical standalone article slug | First article starting on this page, otherwise last preceding article; `none` on contents pages |
| `page` | `none` | Printed page number (starting at 1 after a front cover) |

Return native content such as text, links, and images. Use `info.target` for
format-specific styling; PDF does not interpret CSS. In HTML, `info.title` and
literal metadata placeholders such as `{author}` resolve during composition;
`get-metacontent` must not be nested in this reserved layout metadata. For PDF,
use the supplied dictionary or static content rather than article helpers in
running furniture. Avoid article embeds and footnotes in headers/footers.

HTML page furniture is rendered once per standalone article, not per embedded
occurrence. The bundled schema places it at `<page-header />` and
`<page-footer />` slots, wrapped in `<header class="page-header">` and
`<footer class="page-footer">`. Custom schemas can move those slots. Schemas
without slots get a fallback immediately before/after `<content />`. Returning
`[]` leaves that part empty. Existing navigation, backlinks, and references are
not replaced; the old `<footer>` component slot still controls relationship
footers. The metadata names `__typsite-page-header` and
`__typsite-page-footer` are reserved.

PDF uses the functions on every contents/body/appendix page, never on covers.
Its header always preserves the footnote reset and the native template header.
If both a template header and shared header are present, the template header
comes first, followed by the shared header on a new paragraph. Contextual
headers are evaluated on their own page. With no explicit header, top-aligned
automatic page numbering is preserved too. An explicit shared header takes the
place of automatic header numbering, matching Typst's explicit-header behavior.
A shared footer replaces the template/default footer, so include `info.page` if
you want numbers; without a shared footer the template footer is unchanged. Page
margins, header/footer ascent/descent, typography, and alignment remain normal
Typst settings in `--template lib/pdf.typ`. Layout edits invalidate both
formats; failed layout evaluation preserves their last successful outputs.
Existing sites need the updated `root/lib/lib.typ` schema helper to opt in.

## Styling and print-specific content

`--template lib/pdf.typ` loads a source-root module exporting
`book-style(body)`. The initialized project includes an editable example:

```typ
#let book-style(body) = {
  set page(paper: "a4", margin: 22mm, numbering: "1")
  set text(font: "Inria Sans", size: 10pt)
  set par(justify: true)
  body
}
```

Use normal Typst page, text, figure, and math rules. `--title` sets PDF document
metadata, not the entry article's rich heading. The default metadata title is
`site.title`, falling back to the entry slug.

Fonts in `.typsite/assets/fonts` are passed to Typst. `TYPST_FONT_PATHS` is also
supported and fingerprinted, including symlinked font projections. For
reproducible builds, use explicit fonts, `TYPST_IGNORE_SYSTEM_FONTS=true`, and a
fixed `SOURCE_DATE_EPOCH`. System-font installations are not a tracked project
input; clear the PDF cache after changing them.

PDF is not an HTML/CSS renderer. Bundled common text/alignment/container helpers
have native fallbacks; web scripts/styles are omitted, iframes become media
links, and HTML-only images become visible placeholders. Use native `image` for
print images. Custom HTML helpers and rewrites may need explicit native
fallbacks. Unknown rewrite helpers retain their body, not their web behavior.
Large diagrams/tables may need print-specific sizing.

`inline(...)` uses the same sizing contract in both targets, while keeping
native vectors in PDF rather than rasterizing them:

- `fit-font: false` preserves intrinsic point dimensions, multiplied by `scale`.
  At the browser's standard 16px root size, 1pt maps to 4/3 CSS pixels.
- `fit-font: true` fits the smaller dimension to the surrounding font's em;
  `fit-font: 2em` fits it to two em. Both retain the aspect ratio and honor `scale`.
- PDF scaling reflows the layout box, so surrounding text reserves the scaled
  width and height. HTML frame dimensions use a fixed calibration independent
  of the article's Typst font size and work with current and older frame markup.

Native `inline` frames shrink to the available text-column width by default.
Use `max-width: 130mm` and/or `max-height: 120mm` for tighter print limits; these
are shrink-only limits, so smaller figures are not enlarged. `max-width: none`
disables the width limit. Limits apply only to PDF, leaving web CSS responsive.
The demo support table and pipeline have conservative print-specific limits.
Nested `html.text(size: 100%)[...]` inherits the enclosing PDF text size instead
of resetting it, keeping the LaTeX and Typst wordmarks in proportion.

Responsive CSS can still constrain web diagrams, and changing the browser's
root font size intentionally resizes HTML's rem-based intrinsic frames.

Heading embeds are normalized **before layout**, so the removed wrapper cannot
leave a blank outline entry or increment a counter twice. They work in ordinary
source sequences, native style wrappers, and supported block/box containers.
**All article headings** must pass this pre-layout normalization. A normal
heading or heading embed produced inside an opaque layout `context` (or an
unsupported container) fails explicitly rather than publishing misleading
numbers such as `1.0.1`, an incorrect hierarchy, or a phantom outline entry.
Move the heading outside `context` and keep only its content contextual:

```typ
== Context section
#context [Content using contextual values.]
```

For a dynamic embed inside `context`, use a plain
`embed(..., heading-level: 1)` instead of a heading wrapper. HTML behavior is
unchanged. Nonconverging Typst layouts are rejected, not published as incomplete
books.

## Caching, safety, and failure recovery

The driver is sent on stdin. No generated files are placed in the source tree,
and `--root` is not widened to admit `.typsite`, cache, or external source files.
Source asset paths retain their original Typst file context.

PDF has a separate versioned cache under `.cache/pdf`. Its key covers the
engine/Typst identity, source catalog, ordered roots, print options/template,
project/package/font inputs, and the actual Typst read set. Newly discovered
external imports require a verification compilation. Source changes racing a
build fail instead of checkpointing mixed generations. Invalid caches rebuild
without a manual clean. Invalidation conservatively includes the whole source
and configuration trees, not only the selected forest.

A verified no-op does not rewrite the book or manifest. A missing/damaged output
can be restored from a verified cached PDF without recompiling. Failed export,
layout, or validation retains the last successful book. Publication uses atomic
single-file replacement with syncing. Cross-process cache/output locks prevent
conflicting builds.

A standalone book does **not** replace `.typsite-outputs.json`, remove HTML
artifacts, or claim ownership of the HTML output set. Writing over an artifact
already owned by an HTML build is rejected. Prefer the separate `book/` output
root; a future HTML build could otherwise introduce a same-named asset. Changing
the PDF output path does not delete the previous standalone PDF. `clean` recognizes
a PDF-only cache, but standalone book files must be removed explicitly.

## Upgrading helpers

Back up your customized `root/lib`, then generate a reference project:

```sh
typsite init --dir ../typsite-pdf-reference
```

Merge the following files from the reference project's **`root/lib/`** into
your existing `root/lib/`, retaining your customizations:

- **Required helper set:** `lib.typ`, `site.typ`, `html.typ`, `rewrite.typ`,
  `rule.typ`, and **`links.typ`**. Copy the new `links.typ` even if you only build
  HTML: `site.typ` and `rule.typ` import it. Omitting it breaks those imports.
- **Optional PDF style:** `pdf.typ`; it also imports `links.typ`. Add it if you
  use `--template lib/pdf.typ`.
- **Optional page furniture:** `layout.typ` if you enable `[layout].module`, plus
  `front-cover.typ` / `back-cover.typ` if you want the example covers. A custom
  layout or cover can be used instead.
- **HTML configuration:** merge `.typsite/assets/schemas/page.css` for link and
  furniture styling. Merge the `<page-header />` / `<page-footer />` slots from
  `.typsite/schemas/page.html` if you want their explicit placement; old schemas
  without slots retain the content-adjacent fallback. Add `[layout]` only when
  opting into shared furniture; its absence remains valid.

In particular, the PDF path of a custom schema must delegate to the package's
native schema **before** hiding the body in an HTML-only context or constructing
HTML head content. Keep `@local/typsite:0.1.0`; upgrading the executable does not
change that package identity. Article embed calls do not need migration.
Blindly keeping old HTML-only helper internals does not make a site print-ready.
Run both `typsite compile` and your `typsite pdf` command and check their exit
statuses after merging; the old published outputs are retained on failure.

## Verification

```sh
cargo test --test pdf --locked
cargo test --test typst_pdf --locked -- --ignored --test-threads=1
cargo test --test typst_latest --locked -- --ignored --test-threads=1
# Optional real HTTPS download/cache test (not required by offline CI):
cargo test --test pdf_fonts_network --locked -- --ignored
```

The native suite uses isolated package namespaces and Typst-side assertions,
without requiring a PDF text-extraction utility. It checks article order,
occurrence-local metadata/anchors, rich/native math, relative images, exact
heading/counter behavior, deep nesting, ordered/standalone appendices, external
links, templates, failure recovery, cache no-ops, clean-build equivalence, the
initialized demo, and the CLI. CI runs it on the same platforms/Typst versions as
the HTML compatibility suite. The released/downloaded executable and Nix binary
also run the [HTML/PDF release smoke harness](releasing.md): real demo books,
ordered appendices, covers, shared/template furniture, no-ops, edits,
last-good recovery, HTML isolation, and clean-build byte equivalence. Release
validation requires PDF evidence on every native platform; a configured CI job
without an observed green exact-revision run is still pending.
