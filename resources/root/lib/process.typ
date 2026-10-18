// A square, dependency-free diagram shared by the demo homepages and examples.
// The numbered path snakes across three rows: →, then ←, then →.
#let process-diagram() = {
  set text(font: "Inria Sans", size: 9pt, fill: rgb("243247"))
  set par(leading: 0.45em)

  let ink = rgb("243247")
  let blue = rgb("2563a6")
  let orange = rgb("a65b17")
  let purple = rgb("7353a6")
  let green = rgb("237652")
  let red = rgb("a53b45")
  let muted = rgb("607086")

  let at(x, y, content) = place(top + left, dx: x, dy: y, content)
  let note(body, tint: muted) = text(size: 8pt, weight: "bold", fill: tint, body)
  let card(number, title, body, tint: blue) = rect(
    width: 196pt,
    height: 110pt,
    inset: 9pt,
    radius: 6pt,
    fill: tint.lighten(96%),
    stroke: 0.7pt + tint.lighten(40%),
    stack(
      dir: ttb,
      spacing: 5pt,
      grid(
        columns: (auto, 1fr),
        column-gutter: 5pt,
        text(size: 9.5pt, weight: "bold", fill: tint, number),
        text(size: 10.5pt, weight: "bold", fill: tint, title),
      ),
      body,
    ),
  )
  let arrow(x, y, direction) = at(x, y, text(size: 17pt, fill: muted, direction))
  let recovery(title, body) = rect(
    width: 310pt,
    height: 84pt,
    inset: 9pt,
    radius: 6pt,
    fill: red.lighten(97%),
    stroke: 0.6pt + red.lighten(60%),
    stack(
      dir: ttb,
      spacing: 4pt,
      text(size: 9pt, weight: "bold", fill: red, title),
      text(size: 8.5pt, body),
    ),
  )

  // Explicit equal dimensions keep the aspect ratio square no matter how the
  // cards' text reflows.
  rect(width: 660pt, height: 660pt, inset: 0pt, radius: 10pt,
    fill: white, stroke: 0.6pt + rgb("d8e1ec"), [
    #at(20pt, 18pt, text(size: 21pt, weight: "bold", fill: ink)[Typsite build pipeline])
    #at(20pt, 46pt, text(size: 9pt, fill: muted)[Selective Typst exports · fresh graph on dirty builds · byte-sensitive publication])
    #at(20pt, 63pt, note([FOLLOW 01 → 09 · the middle row runs right to left]))

    // Row 1 →: discover, export, then decide whether anything must be published.
    #at(14pt, 84pt, card("01", [Validate & discover], [
      Lock cache + output roots.\
      Load project-local config, schemas, rewrites, themes, and syntaxes.\
      Rayon: hash articles, libraries, assets, fonts, and package sources.
    ]))
    #arrow(212pt, 129pt, [→])
    #at(232pt, 84pt, card("02", [Export or reuse], [
      #note([HIT], tint: green) → reuse verified raw HTML.\
      #note([MISS], tint: orange) → prepare packages once; bounded parallel Typst exports.\
      JSON deps + before/after hashes.\
      #note([Checkpoint A: shared-dep cache])
    ], tint: orange))
    #arrow(430pt, 129pt, [→])
    #at(450pt, 84pt, card("03", [Published site current?], [
      Check build fingerprint, output hashes, and ownership journal.\
      #note([YES → STOP: true no-op], tint: green)\
      No parse/render, no writes, no reload.\
      #note([NO → continue below], tint: blue)
    ]))
    #arrow(538pt, 198pt, [↓])
    #at(560pt, 206pt, note([dirty only]))

    // Row 2 ←: parse, resolve, then compose. Numbers still ascend leftwards.
    #at(450pt, 232pt, card("04", [Parse article facts], [
      Rayon: parse current raw exports.\
      Extract metadata, headings, anchors, citations, direct embeds, and rewrite
      instructions.\
      #note([No persisted parsed IR or graph])
    ], tint: purple))
    #arrow(430pt, 277pt, [←])
    #at(232pt, 232pt, card("05", [Resolve the graph], [
      Rebuild parents, children, references, and backlinks.\
      Removed edges stay removed.\
      Reject embed / parent / metadata cycles; allow ordinary citation cycles.
    ], tint: purple))
    #arrow(212pt, 277pt, [←])
    #at(14pt, 232pt, card("06", [Compose pages & feeds], [
      Metadata → embeds → rewrites.\
      Rayon: recompose every page with schemas, sidebars, and footers.\
      RSS: direct embeds of marked containers. Sitemap + static assets.
    ]))
    #arrow(110pt, 344pt, [↓])

    // Row 3 →: plan, publish, report.
    #at(14pt, 380pt, card("07", [Validate output plan], [
      Reject traversal, reserved paths, symlinks, and output collisions.\
      Recheck inputs + dependencies.\
      Racing edit → invalidate the export checkpoint and retry.
    ], tint: orange))
    #arrow(212pt, 425pt, [→])
    #at(232pt, 380pt, card("08", [Publish changed bytes], [
      Journal old + intended ownership.\
      Delete stale owned files; preserve unrelated files. Parallel atomic writes.\
      Finalize ownership + hashes.\
      #note([Checkpoint B: published state])
    ], tint: green))
    #arrow(430pt, 425pt, [→])
    #at(450pt, 380pt, card("09", [Report & preview], [
      Exports / renders / writes / removals + stage diagnostics.\
      CLI failures exit nonzero.\
      Successful changes → every client reloads. No-op / failure → no reload.\
      #note([Watch: poll 500 ms; await shutdown])
    ], tint: green))

    // Recovery is a separate band: failures never join the success path.
    #at(20pt, 500pt, text(size: 9pt, weight: "bold", fill: red)[WHEN IT FAILS])
    #at(14pt, 518pt, recovery([STEPS 01–07 FAIL → keep the last-good site], [
      Export / parse / resolve / render errors never publish stale HTML.
      Fix and rerun; successful raw exports remain reusable.
    ]))
    #at(336pt, 518pt, recovery([STEP 08 FAILS → repair on the next run], [
      Publication is atomic per file, not a whole-site transaction, so complete
      old/new files may mix. Keep #raw(".typsite-outputs.json") to recover safely.
    ]))
    #at(20pt, 618pt, text(size: 8pt, fill: muted)[Checkpoint A is disposable cache · output ownership is durable · no speculative scheduler])
  ])
}
