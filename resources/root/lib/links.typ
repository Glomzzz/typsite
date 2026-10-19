// Shared, editable link presentation. HTML uses the existing CSS classes;
// PDF uses native text/underlines, preserving real link destinations.
#import "@local/typsite:0.1.0": html-bindings as html

// Edit these for print. HTML colors/hover behavior remain in page.css.
#let local-color = rgb("2b3a4f")
#let external-color = rgb("2b3a4f")
#let local-stroke = (paint: local-color, thickness: 0.45pt, dash: "dotted")
#let external-stroke = (paint: external-color, thickness: 0.45pt)

#let local-style(body) = context {
  if target() == "html" { html.tag("span", class: "link local", body) }
  else { text(fill: local-color, underline(stroke: local-stroke, body)) }
}

#let external-style(body) = context {
  if target() == "html" { html.tag("span", class: "link external", body) }
  else { text(fill: external-color, underline(stroke: external-stroke, body)) }
}

#let is-external(destination) = {
  type(destination) == str and (destination.starts-with("//") or
    destination.contains(regex("^[A-Za-z][A-Za-z0-9+.-]*:")))
}

// Use in a native template: show: style-links
// Native local locations, labels, and relative/root paths are local links;
// HTTP(S), mailto, and other URI schemes are external.
#let style-links(body) = {
  show link: it => {
    if is-external(it.dest) { external-style(it) } else { local-style(it) }
  }
  body
}
