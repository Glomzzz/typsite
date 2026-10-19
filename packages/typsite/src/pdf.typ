// Native book backend. HTML helpers opt in through the CLI's sys.input so
// semantic markers stay visible to heading show rules (not hidden in context).
#let enabled = sys.inputs.at("typsite-pdf", default: "false") == "true"
#let marker(kind, ..fields) = metadata((typsite-pdf: kind, ..fields.named()))
#let kind(value) = if type(value) == dictionary { value.at("typsite-pdf", default: none) }
#let records(name) = query(metadata).filter(it => kind(it.value) == name)

// Keep this normalization in parity with Rust source/relationship resolution.
// Loading uses only the supplied catalog, never arbitrary include strings.
#let canonical-source(current, reference) = {
  assert(type(reference) == str, message: "PDF article references must be strings")
  assert(not reference.contains("\\") and not reference.contains(":"), message: "Invalid PDF article reference: " + reference)
  let base = if reference.starts-with("/") { () } else { current.split("/").slice(0, -1) }
  let parts = ()
  for part in base + reference.split("/") {
    if part == "" or part == "." { continue }
    if part == ".." {
      assert(parts.len() > 0, message: "PDF article reference escapes the source root: " + reference)
      let _ = parts.pop()
    } else { parts.push(part) }
  }
  assert(parts.len() > 0, message: "Empty PDF article reference")
  let last = parts.pop()
  let extension = last.split(".")
  parts.push(if extension.len() > 1 and not (extension.len() == 2 and extension.first() == "") {
    extension.slice(0, -1).join(".")
  } else { last })
  "/" + parts.join("/")
}

#let find-scope(source, preferred: none) = {
  if preferred != none and preferred.source == source { return preferred }
  let matches = records("article").filter(it => it.value.scope.source == source)
  if matches.len() > 0 { matches.first().value.scope }
}

#let parent-scope(env, scope) = {
  let explicit = records("parent-record").filter(it => it.value.scope.occurrence == scope.occurrence)
  if explicit.len() > 0 {
    let source = canonical-source(scope.source, explicit.last().value.slug)
    let parent = find-scope(source)
    assert(parent != none, message: "PDF metadata parent " + source + " is not in the book; add it as an appendix")
    return parent
  }
  if scope.parent != none { return scope.parent }
  if env.default-parent != none {
    // The Rust driver supplies an already canonical slug (which may contain
    // dots); normalizing it again would strip part of the article basename.
    let source = env.default-parent
    if source == scope.source { return none }
    let parent = find-scope(source)
    assert(parent != none, message: "Default PDF metadata parent " + source + " is not in the book; add it as an appendix")
    return parent
  }
}

#let meta(backend, env, scope, key, from: "$self", seen: ()) = context {
  let scope = if from == "$self" { scope }
    else if from == "parent" { parent-scope(env, scope) }
    else {
      let source = canonical-source(scope.source, from)
      assert(source in env.pages, message: "Unknown PDF metadata article: " + source)
      let target = find-scope(source, preferred: scope)
      assert(target != none, message: "PDF metadata article " + source + " is not in the book; add it as an appendix")
      target
    }
  if scope == none { return env.defaults.at(key, default: []) }
  let token = (scope.source, key)
  assert(not seen.contains(token), message: "Recursive PDF metadata lookup: " + repr(seen + (token,)))
  let seen = seen + (token,)
  let matches = records("meta-record").filter(it => it.value.scope.occurrence == scope.occurrence and it.value.key == key)
  if matches.len() > 0 {
    (backend.decorate)(backend, env, scope, matches.last().value.body, seen: seen, headings: false)
  } else {
    let parent = parent-scope(env, scope)
    if parent == none { env.defaults.at(key, default: []) }
    else { meta(backend, env, parent, key, seen: seen) }
  }
}

#let anchor-link(scope, id, body) = context {
  let matches = records("anchor-record").filter(it => it.value.scope.occurrence == scope.occurrence and it.value.id == str(id))
  assert(matches.len() == 1, message: "PDF anchor '" + str(id) + "' must occur exactly once in " + scope.source + " (this occurrence)")
  link(matches.first().location(), body)
}

#let citation(backend, env, scope, slug, body: none, anchor: "") = context {
  let source = canonical-source(scope.source, slug)
  assert(source in env.pages, message: "Cited PDF article " + slug + " not found in " + scope.source)
  let target = find-scope(source, preferred: scope)
  if target == none {
    assert(source in env.urls, message: "Cited article " + source + " is outside the PDF; add it as an appendix or configure site.base_url")
    let url = env.urls.at(source) + if anchor == "" { "" } else { "#" + anchor }
    // Do not silently erase title-only citations to pages outside the forest.
    // Their canonical slug is an explicit fallback; rich titles live in-book.
    return link(url, if body == none { text(source) } else { body })
  }
  let body = if body == none { meta(backend, env, target, "title") } else { body }
  if anchor != "" { return anchor-link(target, anchor, body) }
  let matches = records("article").filter(it => it.value.scope.occurrence == target.occurrence)
  link(matches.first().location(), body)
}

// Named definitions are data, not footnotes at their declaration site. Each
// page's first reference creates the native note; later references share it.
#let note-reference(backend, env, scope, id) = {
  marker("note-reference", scope: scope, id: id)
  context {
    let definitions = records("note-definition").filter(it => it.value.scope.occurrence == scope.occurrence and it.value.id == id)
    assert(definitions.len() == 1, message: "PDF footnote '" + id + "' must be defined exactly once in " + scope.source)
    let all = records("note-reference")
    let local = all.filter(it => it.value.scope.occurrence == scope.occurrence and it.value.id == id and it.location().page() == here().page())
    let first = local.first()
    let previous = query(selector(metadata).before(here())).filter(it => kind(it.value) == "note-reference" and it.value.scope.occurrence == scope.occurrence and it.value.id == id).last()
    let index = all.position(it => it.location() == first.location())
    let target = label("typsite-pdf-note-" + str(index))
    if previous.location() == first.location() {
      let body = (backend.decorate)(backend, env, scope, definitions.first().value.body, headings: false)
      [#footnote(body) #target]
    } else { footnote(target) }
  }
}

// Only a marker occupying the heading body is a heading embed. Do not discard
// unrelated text, or mistake an embed buried in arbitrary markup for a chapter.
#let heading-embed(body) = {
  if body.func() == metadata and kind(body.value) == "embed" { return body.value }
  if body.has("children") {
    let children = body.children.filter(it => not (it.func() == [ ].func()))
    if children.len() == 1 { return heading-embed(children.first()) }
  }
}

// Remove heading wrappers before layout, rather than replacing a located
// heading in a show rule (which would leave a phantom outline/counter entry).
#let prepare(body) = {
  if type(body) != content { return body }
  if body.func() == footnote {
    if type(body.body) == label { return marker("note-ref", id: str(body.body)) }
    let id = body.at("label", default: none)
    if id != none { return marker("note-def", id: str(id), body: body.body) }
  }
  if body.func() == heading {
    let data = heading-embed(body.body)
    if data != none {
      return marker("embed", slug: data.slug,
        heading-level: body.at("level", default: body.at("depth", default: 1)) + body.at("offset", default: 0))
    }
  }
  if body.has("child") and body.has("styles") {
    return (body.func())(prepare(body.child), body.styles)
  }
  if body.func() == [].func() {
    return body.children.map(prepare).join()
  }
  if (block, box, quote, list.item, enum.item).contains(body.func()) {
    let fields = body.fields()
    let child = fields.remove("body")
    let id = fields.remove("label", default: none)
    let rebuilt = (body.func())(prepare(child), ..fields)
    return if id == none { rebuilt } else { [#rebuilt #id] }
  }
  body
}

#let placement(scope, element, option) = {
  if type(option) == int {
    assert(option > 0, message: "PDF heading level must be positive")
    return scope.level + option
  }
  assert(type(option) == str, message: "PDF heading-level must be 'child', 'peer', or a positive integer")
  if type(option) == str and option != "child" and option != "peer" {
    let number = int(option)
    assert(number > 0, message: "PDF heading level must be positive")
    return scope.level + number
  }
  // selector.before uses document order, including multiple headings on a page.
  let before = query(selector(metadata).before(element.location())).filter(it => kind(it.value) == "heading-record" and it.value.scope.occurrence == scope.occurrence)
  let previous = if before.len() == 0 { scope.level } else { before.last().value.level }
  if option == "peer" { previous } else { previous + 1 }
}

#let decorate(backend, env, scope, body, seen: (), headings: true) = {
  let expand(data, occurrence, level) = (backend.render)(
    backend, env, canonical-source(scope.source, data.slug),
    stack: scope.stack, parent: scope, occurrence: occurrence, level: level,
  )
  // Capture source identity lexically. Layout state must not discover one more
  // source per pass: deep, dynamically generated forests need to work too.
  show metadata: it => {
    let data = it.value
    let name = kind(data)
    if name == "embed" {
      context {
        let level = placement(scope, it, data.heading-level)
        marker("heading-record", scope: scope, level: level)
        expand(data, it.location(), level)
      }
    } else if name == "set" {
      marker("meta-record", scope: scope, key: data.key, body: data.body)
    } else if name == "get" {
      meta(backend, env, scope, data.key, from: data.from, seen: seen)
    } else if name == "parent" {
      marker("parent-record", scope: scope, slug: data.slug)
    } else if name == "anchor" {
      marker("anchor-record", scope: scope, id: str(data.id))
    } else if name == "goto" {
      anchor-link(scope, data.id, data.body)
    } else if name == "note-def" {
      marker("note-definition", scope: scope, id: data.id, body: data.body)
    } else if name == "note-ref" {
      note-reference(backend, env, scope, data.id)
    } else if name == "rewrite" {
      let attrs = data.attrs
      if data.id == "cite" or data.id == "cite-with-title" {
        citation(backend, env, scope, attrs.slug,
          body: if data.id == "cite-with-title" { none } else { data.body },
          anchor: attrs.at("anchor", default: ""))
      } else if data.id == "code-inline" or data.id == "code-block" {
        raw(attrs.at("content", default: ""), lang: attrs.at("lang", default: none), block: data.id == "code-block")
      } else if data.id == "footnote-def" {
        if attrs.name == "!numbering" { footnote(data.body) }
        else { marker("note-definition", scope: scope, id: attrs.name, body: data.body) }
      } else if data.id == "footnote-ref" or data.id == "footnote-ref-svg" {
        note-reference(backend, env, scope, attrs.name)
      } else { data.body }
    } else { it }
  }
  show footnote: it => {
    if type(it.body) == label {
      if str(it.body).starts-with("typsite-pdf-note-") { return it }
      return note-reference(backend, env, scope, str(it.body))
    }
    let id = it.at("label", default: none)
    assert(id == none or str(id).starts-with("typsite-pdf-note-"), message: "PDF footnote definition inside opaque context: move the named definition to the article's top level")
    it
  }
  show link: it => {
    if type(it.dest) == label { anchor-link(scope, str(it.dest), it.body) } else { it }
  }
  show ref: it => context {
    let notes = records("note-definition").filter(note => note.value.scope.occurrence == scope.occurrence and note.value.id == str(it.target))
    if notes.len() > 0 { return note-reference(backend, env, scope, str(it.target)) }
    if it.supplement != auto and type(it.supplement) == content {
      let targets = query(it.target)
      // Supplements customize native numbered refs; they are not the complete
      // link body. Keep Typst's number/supplement logic for those targets.
      if targets.any(target => (heading, math.equation, figure, footnote).contains(target.func())) { return it }
      anchor-link(scope, str(it.target), it.supplement)
    } else { it }
  }
  // Native labels remain untouched, so native numbered references keep Typst's
  // normal behavior. This index additionally lets Typsite links use local IDs.
  show selector.or(heading, par, text, strong, list, enum, emph, overline, underline, super, sub, raw, link, footnote, math.equation, highlight, align, strike, terms, figure, box, block): it => {
    let id = it.at("label", default: none)
    if id != none { marker("anchor-record", scope: scope, id: str(id)) }
    it
  }
  if headings {
    show heading: set heading(offset: scope.level)
    show heading: it => {
      if it.at("label", default: none) == label("typsite-pdf-title") { return it }
      let data = heading-embed(it.body)
      assert(data == none, message: "PDF heading embed inside opaque context: move the heading embed out of context, or use a plain #embed with heading-level")
      {
        marker("heading-record", scope: scope, level: it.level)
        it
      }
    }
    body
  } else { body }
}

#let render(backend, env, source, stack: (), parent: none, occurrence: "entry", level: 1) = {
  assert(source in env.pages, message: "Embedded PDF article " + source + " not found via " + repr(stack))
  assert(not stack.contains(source), message: "Recursive PDF embed: " + repr(stack + (source,)))
  assert(level > 0, message: "PDF heading level must be positive")
  let scope = (source: source, occurrence: occurrence, stack: stack + (source,), parent: parent, level: level)
  marker("article", scope: scope)
  marker("heading-record", scope: scope, level: level)
  [#heading(level: level, meta(backend, env, scope, "title")) #label("typsite-pdf-title")]
  decorate(backend, env, scope, prepare(env.pages.at(source)()))
}

// Exported by the package as pdf-book; normal authors keep using schema/embed.
#let book(pages: (:), entry: "/index", appendices: (), title: "", defaults: (:), default-parent: none, urls: (:)) = {
  assert(enabled, message: "Use typsite pdf to prepare a native book")
  let backend = (render: render, decorate: decorate)
  let env = (pages: pages, defaults: defaults, default-parent: default-parent, urls: urls)
  set document(title: if title == "" { entry } else { title })
  set page(header: counter(footnote).update(0))
  outline(title: [Contents])
  pagebreak(weak: true)
  render(backend, env, entry)
  if appendices.len() > 0 {
    pagebreak(weak: true)
    counter(heading).update(0)
    set heading(numbering: "A.1")
    for (index, source) in appendices.enumerate() {
      if index > 0 { pagebreak(weak: true) }
      render(backend, env, source, occurrence: "appendix-" + str(index))
    }
  }
}
