// Shared native content for HTML articles and PDF running page furniture.
// Enable in .typsite/options.toml: [layout] module = "lib/layout.typ"
// info: (target: "html" | "pdf", source: str | none, title: content, page: int | none)
// Keep this module independent of lib.typ to avoid a schema import cycle.
#let page-header(info) = {
  link("https://github.com/Glomzzz/typsite")[Collected notes]
}

#let page-footer(info) = {
  if info.target == "pdf" {
    align(center)[#info.title #h(1em) #info.page]
  } else {
    [Made with #link("https://github.com/Glomzzz/typsite")[Typsite] and Typst.]
  }
}
