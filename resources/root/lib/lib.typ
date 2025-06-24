// Site lib, feel free to customize your own site
#import "@local/typsite:0.1.0": (
  inline,
  embed,
  metacontent,
  get-metacontent,
  title,
  page-title,
  taxon,
  author,
  date,
  heading-numbering,
  sidebar,
  parent,
  unique
)

#import "html.typ" as html

#import "site.typ" : (
  details, 
  inline-math,
  mathml-or-inline,
  auto-filter,
  block-quote,
  note,
)

#import "rewrite.typ" :  (
  cite, cite-title
)


/// Schema for a article page
/// Usage:
/// ```typ
/// #show : schema.with("page")
/// 
/// #Your article content 
/// ```
/// - name(str): one of file names in `.typsite/schemas/`
///     The name of the schema
/// - head(content):
///     Custom head of a article
/// - body(content):
///     The body of the article
/// -> HTML document with a schema ~> HTML Page
#let schema(name,head:[], body) = {
  import "@local/typsite:0.1.0": schema
  schema(
    name,
    body,
    [
      #import "@local/typsite:0.1.0": mathyml
      #unique(mathyml.include-mathfont())
      #head
    ],
    body => {
      import "rule.typ": *
      show: rule-decorate
      show: rule-equation-mathyml-or-inline
      show: rule-footnote
      show: rule-link-common
      show: rule-link-anchor
      show: rule-ref-footnote
      show: rule-ref-label
      show: rule-raw
      show: rule-label
      body
    },
  )
}

#let _LaTeX = {
  let A = (
    offset: (
      x: -0.33em,
      y: -0.3em,
    ),
    size: 0.7em,
  )
  let T = (
    x_offset: -0.12em,
  )
  let E = (
    x_offset: -0.2em,
    y_offset: 0.23em,
    size: 1em,
  )
  let X = (
    x_offset: -0.1em,
  )
  box(height: 0.92em,[L#h(A.offset.x)#text(size: A.size, baseline: A.offset.y)[A]#h(T.x_offset)T#h(E.x_offset)#text(size: E.size, baseline: E.y_offset)[E]#h(X.x_offset)X])
}

#let LaTeX = auto-filter(inline(_LaTeX, fit-font: true))
