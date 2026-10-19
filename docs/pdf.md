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

The book adds an article-title heading above each article. Its local `=` headings
are one level below that heading. Heading embeds use their source heading depth;
plain embeds support `heading-level: "child"`, `"peer"`, or a positive numeric
local level. A source that intentionally skips levels can produce numbering such
as `1.2.0.1`; the renderer does not silently renumber its structure.

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

Heading embeds are normalized **before layout**, so the removed wrapper cannot
leave a blank outline entry or increment a counter twice. They work in ordinary
source sequences, native style wrappers, and supported block containers. A
heading embed produced inside an opaque layout `context` cannot be normalized;
it fails explicitly. Move that heading out of `context`, or use a plain
`embed(..., heading-level: 1)` inside it. Nonconverging Typst layouts are rejected,
not published as incomplete books.

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

Merge `lib.typ`, `site.typ`, `html.typ`, and `rewrite.typ` rather than overwriting
customizations. In particular, the PDF path of a custom schema must delegate to
the package's native schema **before** hiding the body in an HTML-only context or
constructing HTML head content. The new sample `lib/pdf.typ` is optional. Article
embed calls do not need migration; blindly keeping old HTML-only helper
internals does not make a site print-ready.

## Verification

```sh
cargo test --test pdf --locked
cargo test --test typst_pdf --locked -- --ignored
cargo test --test typst_latest --locked -- --ignored
```

The native suite uses isolated package namespaces and Typst-side assertions,
without requiring a PDF text-extraction utility. It checks article order,
occurrence-local metadata/anchors, rich/native math, relative images, exact
heading/counter behavior, deep nesting, ordered/standalone appendices, external
links, templates, failure recovery, cache no-ops, clean-build equivalence, the
initialized demo, and the CLI. CI runs it on the same platforms/Typst versions as
the HTML compatibility suite.
