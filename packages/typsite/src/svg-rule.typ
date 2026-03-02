#import "typsite.typ": inline-content, rewrite
#let inline-anchor(label, body) = link("#anchor:" + str(label))[#body]
#let inline-goto(label, body) = link("#goto:" + str(label))[#underline(
  stroke: stroke(thickness: 1pt, dash: "densely-dotted"),
  body,
)]
#let inline-footnote-ref(index, id) = context {
  super(link("#footnote-ref:" + str(id))[#{ index + 1 }])
}



// Use at first
#let rule-rewrite-link(content) = {
  show link: it => {
    let dest = it.dest
    let dest-type = type(dest)
    if dest-type == label {
      underline(stroke: stroke(thickness: 1pt, dash: "densely-dotted"), inline-goto(dest, it.body))
    } else {
      if (
        it.body.func() != underline
          and (
            not (
              it.dest.starts-with("#footnote-ref:") or it.dest.starts-with("#anchor:") or it.dest.starts-with("#goto:")
            )
          )
      ) {
        underline(stroke: stroke(thickness: 1pt), it)
      } else {
        it
      }
    }
  }
  content
}

// Use before `rule-ref-label`
#let rule-ref-footnote(footnotes) = content => {
  show ref: it => context {
    let target = it.target
    for (index, id) in footnotes.enumerate() {
      if id == target {
        return inline-footnote-ref(index, id)
      }
    }
    it
  }
  content
}

#let rule-footnote(footnotes) = content => {
  show footnote: it => context {
    let body = it.body
    if type(body) == label {
      for (index, id) in footnotes.enumerate() {
        if id == body {
          return inline-footnote-ref(index, id)
        }
      }
      it
    } else {
      panic("Footnote definition in inline content is not supported")
    }
  }
  content
}

#let rule-ref-label(content) = {
  show ref: it => {
    let target = it.target
    let supplement = it.supplement

    inline-goto(target, supplement)
  }
  content
}
#let rule-rewrite-label(content) = {
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
  ): it => context {
    let label = it.at("label", default: none)
    if label == none {
      return it
    }
    inline-anchor(label, it)
  }
  content
}
