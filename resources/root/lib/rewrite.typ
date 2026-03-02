// Site lib, feel free to customize your own site

#import "@local/typsite:0.1.0" : *

/// Creates a code snippet inline rewrite in HTML.
///
/// - lang (str):
///     The programming language of the code snippet.
/// - theme (str):
///    The theme to be applied to the code snippet.
/// - content (content):
///    The content of the code snippet.
/// -> rewrite ~> HTML code inline element
#let code-inline(lang: "text", theme: "onedark", content) = {
  let code = rewrite(
    "code-inline",
    lang: str(lang),
    theme: str(theme),
    content: to-str(content),
    content,
  )
  box(code)
}
/// Creates a code snippet block rewrite in HTML.
///
/// - lang (str):
///     The programming language of the code snippet.
/// - theme (str):
///     The theme to be applied to the code snippet.
/// - content (content):
///     The content of the code snippet.
/// -> rewrite ~> HTML code block element
#let code-block(lang: "text", theme: "onedark", content) = {
  rewrite(
    "code-block",
    lang: str(lang),
    theme: str(theme),
    content: to-str(content),
    content,
  )
}

/// Creates a footnote definition rewrite in HTML.
///
/// - name (str):
///     The name of the footnote definition.
/// - content (content):
///     The content of the footnote definition.
/// -> rewrite ~> HTML footnote definition element
#let footnote-def(name, content) = {
  box(rewrite("footnote-def", name: name, content))
}

/// Creates a footnote reference rewrite in HTML.
///
/// - name (str):
///     The name of the footnote reference.
/// - svg (bool):
///     Wether in SVG.
/// -> rewrite ~> HTML footnote reference element
#let footnote-ref(name) = {
  box(rewrite("footnote-ref", name: name)[])
}

/// Creates a footnote reference rewrite in SVG.
///
/// - name (str):
///     The name of the footnote reference.
/// -> rewrite ~> HTML footnote reference element
#let footnote-ref-svg(name) = {
  box(rewrite("footnote-ref-svg", name: name)[])
}

/// Creates a citation rewrite in HTML.
///
/// - slug (str):
///     The slug of the citation, which is used to reference the citation in the document.
/// -> rewrite ~> HTML citation element
#let cite-title(slug) = context {
  if target() == "html" {
    box(rewrite("cite-with-title", slug: str(slug))[])
  } else {
    []
  }
}


/// Creates a citation rewrite in HTML.
///
/// - slug (str):
///     The slug of the citation, which is used to reference the citation in the document.
/// - anchor (str):
///     An optional anchor for the citation, which can be used to link to a specific section
/// -> rewrite ~> HTML citation element
#let cite(slug, anchor: "", content) = context {
  if target() == "html" {
    box(rewrite("cite", slug: str(slug), anchor: anchor, content))
  } else {
    content
  }
}
