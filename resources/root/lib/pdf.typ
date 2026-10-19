// Optional native book style. Use: typsite pdf --template lib/pdf.typ
// Keep this module inside the Typst source root; .typsite is not that root.
#let book-style(body) = {
  set page(paper: "a4", margin: (x: 22mm, y: 20mm), numbering: "1")
  set text(font: "Inria Sans", size: 10pt)
  set par(justify: true)
  set heading(numbering: "1.1")
  show heading.where(level: 1): set text(size: 1.5em)
  body
}
