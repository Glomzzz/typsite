// Optional native book style. Use: typsite pdf --template lib/pdf.typ
// Keep this module inside the Typst source root; .typsite is not that root.
#let book-style(body) = {
  set page(paper: "a4", margin: (x: 22mm, y: 20mm), numbering: "1")
  let chinese = sys.inputs.at("typsite-font-family", default: "")
  let fonts = if chinese == "" { ("Inria Sans",) } else { ("Inria Sans", chinese) }
  set text(font: fonts, size: 10pt, weight: 400)
  set par(justify: true, leading: 0.7em)
  set heading(numbering: "1.1")
  show raw: set text(font: if chinese == "" { ("DejaVu Sans Mono",) } else { ("DejaVu Sans Mono", chinese) })
  show strong: set text(weight: 500)
  // Local headings inherit source text settings; set explicit point sizes so
  // CJK fallback fonts and nested articles cannot inflate the hierarchy.
  show heading: set text(font: fonts, size: 10.5pt, weight: 500)
  show heading.where(level: 1): set text(size: 15pt)
  show heading.where(level: 2): set text(size: 12.5pt)
  show heading.where(level: 3): set text(size: 11pt)
  body
}
