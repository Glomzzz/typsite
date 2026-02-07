// Site lib, feel free to customize your own site
//
#import "rewrite.typ": *
#import "html.typ" as html
#import "@local/typsite:0.1.0" : to-str

#let default-code-highlight-theme = "forest"
#let rule-raw(body) = {
  show raw: it => context {
    if inline-content.get() or target() != "html"  {
      return it
    }
    let text = it.text
    let block = it.block
    let lang = it.lang
    if lang == none {
      lang = "text"
    } else {
      lang = to-str(lang)
    }
    if lang == "typc"{
      return it
    }
    let theme = default-code-highlight-theme
    let split = lang.split("--")
    if split.len() > 1 {
      lang = split.at(0)
      theme = split.at(1)
    }
    if block {
      code-block(text, lang: lang, theme: theme)
    } else {
      code-inline(text, lang: lang, theme: theme)
    }
  }
  body
}

#let rule-equation-inlined(body) = {
  import "./site.typ": inline-math

  show math.equation: it => context {
    if inline-content.get() or target() != "html"  {
      return it
    }
    set text(weight: 500)
    inline-math(it, block: it.block)
  }
  body
}

// https://codeberg.org/akida/mathyml
#let rule-equation-mathyml-try(body) = {
  import "@local/typsite:0.1.0": mathyml
  import mathyml : try-to-mathml

  show math.equation: body => context {
    if inline-content.get() or target() != "html"  {
      return body
    }
    set text(weight: 500)
    try-to-mathml(body)
  }

  body
}

 // Will convert failed-to-mathml equations into inlined-svg
#let rule-equation-mathyml-or-inline(body) = {
  import "./site.typ": mathml-or-inline

  show math.equation: inner => context {
    if inline-content.get() or target() != "html"  {
      return inner
    }
    mathml-or-inline(inner)
  }

  body
}

#let rule-decorate(body) = {
  show emph: it => context {
    if inline-content.get() or target() != "html"  {
      return it
    }
    html.em(it.body)
  }
  show super: it => context {
    if inline-content.get() or target() != "html"  {
      return it
    }
    html.tag("sup", it.body)
  }
  show sub: it => context {
    if inline-content.get() or target() != "html"  {
      return it
    }
    html.tag("sub", it.body)
  }
  show overline: it => context {
    if inline-content.get() or target() != "html"  {
      return it
    }
    html.text-decoration("overline", it.body)
  }
  show underline: it => context {
    if inline-content.get() or target() != "html"  {
      return it
    }
    html.text-decoration("underline", it.body)
  }
  show highlight: it => context {
    if inline-content.get() or target() != "html"  {
      return it
    }
    html.mark(it.fill, it.body)
  }
  body
}
// Use before `rule-ref-label`
#let rule-ref-footnote(footnotes) = body => {
  show ref: it => context {
    if inline-content.get() or target() != "html"  {
      return it.supplement
    }
    let target = it.target
    if footnotes.contains(target) {
      footnote-ref(str(target))
    } else {
      it
    }
  }
  body
}
#let rule-ref-label(body) = {
  import "./site.typ": link-local-style
  show ref: it => context {
    if inline-content.get() or target() != "html"  {
      return it.supplement
    }
    let target = it.target
    let supplement = it.supplement

    link-local-style(goto(target, supplement))
  }
  body
}

#let rule-ref(footnotes) = body => {
  let body = rule-ref-footnote(footnotes)(body)
  rule-ref-label(body)
}


#let rule-footnote(footnotes) = body => {
  show footnote: it => context {
    if inline-content.get() or target() != "html"  {
      return it
    }
    let body = it.body
    if type(body) == label {
      for (index, id) in footnotes.enumerate() {
        if id == body {
          return footnote-ref(str(id))
        }
      }
      return it
    }
    if not it.has("label") {
      return footnote-def("!numbering", body)
    }
    let name = str(it.label)
    footnote-def(name, body)
  }
  body
}

#let rule-link-common(body) = {
  import "./site.typ": link-external, link-local
  show link: it => context {
    if inline-content.get() or target() != "html"  {
      return it
    }
    let dest = it.dest
    let dest-type = type(dest)
    // Common link
    if dest-type == str {
      if dest.starts-with("mailto:") or dest.starts-with("http") {
        link-external(dest, it.body)
      } else {
        link-local(dest, it.body)
      }
    } else {
      it
    }
  }
  body
}
#let rule-link-anchor(body) = {
  show link: it => context {
    if inline-content.get() or target() != "html"  {
      return it
    }
    let dest = it.dest
    let dest_type = type(dest)
    // Label(anchor) link
    if dest_type == label {
      goto(str(dest))
    } else {
      it
    }
  }
  body
}

// Use at last
#let rule-label(body) = {
  show selector.or(
    heading,
    par,
    text,
    strong,
    list,
    emph,
    overline,
    underline,
    super,
    sub,
    raw,
    link,
    footnote,
    math.equation,
    highlight,
    align,
    strike,
    terms,
    figure,
  ): it => {
    let label = it.at("label", default: none)
    if label == none {
      return it
    }
    if it.func() == heading {
      return [#anchor(str(label)) #it]
    }

    [#anchor(str(label)) #it]
  }
  body
}
