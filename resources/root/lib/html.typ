#import "@local/typsite:0.1.0": html-bindings as _html, to-str
#import _html: *

// Keep the generated HTML bindings untouched. These common authoring wrappers
// use the package's target-aware boundary instead of evaluating std.html in PDF.
#let div(body, ..attrs) = _html.tag("div", ..attrs, body)
#let span(body, ..attrs) = _html.tag("span", ..attrs, body)
#let blockquote(body, ..attrs) = context {
  if target() != "html" { return std.quote(block: true, body) }
  _html.tag("blockquote", ..attrs, body)
}
#let a(body, ..attrs) = _html.tag("a", ..attrs, body)
#let img(body, ..attrs) = _html.tag("img", ..attrs, body)

/// Creates a text decoration element.
///
/// - decoration (str): overline | underline | line-through | "$decoration $decoration"
///     The text decoration to apply, i.e. "overline", "overline underline"
/// - content (content):
///     The content to be decorated.
/// -> HTML with text-decoration style
#let text-decoration(decoration, content) = context {
  if target() != "html" {
    let decorated = content
    if decoration.contains("overline") { decorated = overline(decorated) }
    if decoration.contains("underline") { decorated = underline(decorated) }
    if decoration.contains("line-through") { decorated = strike(decorated) }
    return decorated
  }
  span(style: "text-decoration: " + str(decoration) + ";", content)
}

/// Creates a highlight element with a background color.
///
/// - color (color):
///     The background color to apply to the text.
/// - attrs (..):
///     Additional attributes to apply to the HTML element.
/// - content (content):
///     The content to be highlighted.
/// -> HTML with background color style
#let mark(color, ..attrs, content) = context {
  if target() != "html" {
    return std.highlight(fill: color, content)
  }
  tag("mark", style: "background: " + color.to-hex() + ";", ..attrs, content)
}


/// Creates a text element with font styles applied.
///
/// - align (horizontal-alignment): "left" | "center" | "right" | none
///     The horizontal alignment of the text.
///     If none, no alignment is applied.
///     If not none, the text will be wrapped in a div with the specified alignment.
/// - font (str):
///     The font family to use for the text.
/// - style (str): "normal", "italic", "oblique"
///     The font style to apply
/// - weight (str):  "regular", "bold", "bolder", "lighter"
///     The font weight to apply
/// - size (ratio | length):
///     The font size to apply, as a ratio of the default size (e.g., 100% for default size).
/// - fill (color):
///     The text color to apply, as a color value.
/// - tracking (length):
///     The letter spacing to apply, as a length value (e.g., 0pt for no tracking).
/// - spacing (length):
///     The word spacing to apply, as a length value (e.g., 0pt for no spacing).
/// - frame (html-element): "span" | "div" | any HTML element
///     The HTML element to wrap the text in. Defaults to "span" if align is none.
///     If align is not none, the text will be wrapped in a "div" element
/// - content (content):
///    The content to be styled with the specified font properties.
/// -> HTML with font styles applied
#let text(
  align: none,
  font: none,
  style: "normal",
  weight: "regular",
  size: 100%,
  fill: none,
  tracking: 0pt,
  spacing: 0pt + 100%,
  frame: span,
  content,
) = context {
  let text-size = if type(size) == ratio {
    11pt * size
  } else if type(size) == length {
    size * 0.75
  } else {
    panic("Invalid type " + type(size) + " for size: " + to-str([#size]))
  }
  if target() != "html" {
    let attrs = (:)
    if font != none { attrs.insert("font", font) }
    if fill != none { attrs.insert("fill", fill) }
    // Percentages inherit the enclosing native size; resetting nested 100%
    // text to 11pt made a large LaTeX logo dwarf its adjacent Typst wordmark.
    let native-size = if type(size) == ratio { std.text.size * size } else { text-size }
    let native = std.text(style: style, weight: weight, size: native-size,
      tracking: tracking, spacing: spacing, ..attrs, content)
    return if align == none { native } else { std.align(align, native) }
  }
  let content = std.text(
    style: style,
    weight: weight,
    size: text-size,
    tracking: tracking,
    spacing: spacing,
    content,
  )
  if target() != "html" {
    return content
  }
  let styles = ()
  styles.push("vertical-align: baseline;")
  if font != none {
    styles.push("font-family: " + font + ";")
  }
  styles.push("font-style: " + to-str([#style]) + ";")
  if weight != "regular" {
    styles.push("font-weight: " + to-str([#weight]) + ";")
  }
  if size != 100% {
    styles.push("font-size: " + to-str([#size]) + ";")
  }
  if fill != none {
    styles.push("color: " + fill.to-hex() + ";")
  }
  if tracking != 0pt {
    styles.push("letter-spacing: " + to-str([#tracking]) + ";")
  }
  if spacing != 100% + 0pt {
    styles.push("word-spacing: " + to-str([#spacing]) + ";")
  }
  if align != none {
    styles.push("text-align: " + to-str([#align]) + ";")
  }
  let frame = if align == none  {
    frame
  } else {
    div
  }
  frame(style: styles.join(" "), content)
}

/// Creates an HTML element with text alignment applied.
///
/// - alignment (horizontal-aligment): "left" | "center" | "right" | none
///     The horizontal alignment of the content. if none then returns the content as is.
/// - content (content):
///     The content to be aligned.
/// -> HTML with text alignment applied
#let align(alignment, content) = context {
  if target() != "html" {
    return std.align(alignment, content)
  }
  if alignment == none {
    return content
  }
  let horizontally = if alignment == none { "left" } else if alignment == center { "center" } else if (
    alignment == left
  ) { "left" } else if alignment == right { "right" } else if alignment.x == center { "center" } else if (
    alignment.x == right
  ) { "right" } else { "left" }
  div(style: "text-align: " + horizontally + ";", content)
}
