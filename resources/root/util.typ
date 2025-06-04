

// Utils

#let to-string(content) = {
  if type(content) == none {
    return ""
  }
  if type(content) == str {
    return content
  }
  if content.has("text") {
    if type(content.text) == "string" {
      content.text
    } else {
      to-string(content.text)
    }
  } else if content.has("children") {
    content.children.map(to-string).join("")
  } else if content.has("body") {
    to-string(content.body)
  } else if content == [ ] {
    ""
  } else {
    "not-supported"
  }
}

#let bool_to_str(val) = {
  if val {
    "true"
  } else {
    "false"
  }
}

#let text-align(alignment, content) = context {
  if target() != "html" {
    return align(alignment, content)
  }
  let horizontally = if alignment == none { "left" } else if alignment == center { "center" } else if (
    alignment == left
  ) { "left" } else if alignment == right { "right" } else if alignment.x == center { "center" } else if (
    alignment.x == right
  ) { "right" } else { "left" }
  html.elem("div", attrs: (style: "text-align: " + horizontally + ";"))[#content]
}

#let typst_scale = scale


#let inline-content = state("inline-content", false)



#let inline(scale: 100%, alignment: none, fill: none, content) = {
  context {
    let content = if fill != none { block(content, fill: fill) } else { content }
    let content = typst_scale(scale, origin: left + top, content)
    if target() != "html" {
      return align(alignment, content)
    }
    let size = measure(box(content))
    let width = size.width * scale
    let height = size.height * scale
    let width = if width == 0pt { auto } else { width }
    let height = if height == 0pt { auto } else { height }
    let content = [
      #inline-content.update(true)
      #let frame = html.frame(content)
      #html.elem("span", attrs: (class: "auto-svg", scale: to-string([#scale])), frame)
      #inline-content.update(false)
    ]
    text-align(alignment, content)
  }
}

#let codeinline(lang: "text", theme: "onedark", content) = context {
  let lang = lang
  box[#html.elem(
      "rewrite",
      attrs: (id: "code-inline", lang: str(lang), theme: str(theme), content: to-string(content)),
      content,
    )]
}

#let codeblock(lang: "text", theme: "onedark", content) = {
  let lang = lang
  html.elem(
    "rewrite",
    attrs: (id: "code-block", lang: str(lang), theme: str(theme), content: to-string(content)),
    content,
  )
}


#let inline_math(body, block: bool, scale: 100%) = {
  if block {
    html.elem("div", attrs: (class:"math-container"))[
      #html.elem("span", attrs: (class: "math-block", content: to-string(body)))[
        #inline(scale: scale)[#body]
      ]
    ]
  } else {
    box[
      #html.elem("span", attrs: (id: "math-inline", content: to-string(body)))[
        #inline(scale: scale)[#body]
      ]
    ]
  }
}

// Common
#let img(path, width: auto) = context {
  if target() != "html" {
    return image(path, width:width)
  }
  html.elem("img", attrs: (src: str(path), width: to-string([#width])))[ ]
}

#let link_local_style(content) = {
  html.elem("span", attrs: (class: "link local"), content)
}

#let href(url, content) = {
  html.elem("span", attrs: (onclick: "window.location.href = '" + str(url) + "'"), content)
}
#let link_local(url, content) = {
  link_local_style[#href(url, content)]
}
#let link_external(url, content) = {
  html.elem("span", attrs: (class: "link external"))[#href(url, content)]
}

// decoration: overline | underline | line-through | "$decoration $decoration"
#let text-decoration(decoration, content) = {
  html.elem("span", attrs: (style: "text-decoration: " + str(decoration) + ";"))[#content]
}

#let mark(color, content) = {
  html.elem("mark", attrs: (style: "background: " + color.to-hex() + ";"))[#content]
}

#let html-text(
  font: none,
  style: "normal",
  weight: "regular",
  size: 100%,
  fill: none,
  tracking: 0pt,
  spacing: 0pt + 100%,
  content,
) = {
  let styles = ()
  if font != none {
    styles.push("font: " + font + ";")
  }
  if style != "normal" {
    styles.push("font-style: " + to-string([#style]) + ";")
  }
  if weight != "regular" {
    styles.push("font-weight: " + to-string([#weight]) + ";")
  }
  if size != 100% {
    styles.push("font-size: " + to-string([#size]) + ";")
  }
  if fill != none {
    styles.push("color: " + fill.to-hex() + ";")
  }
  if tracking != 0pt {
    styles.push("letter-spacing: " + to-string([#tracking]) + ";")
  }
  if spacing != 100% + 0pt {
    styles.push("word-spacing: " + to-string([#spacing]) + ";")
  }

  html.elem("span", attrs: (style: styles.join(" ")))[#content]
}

