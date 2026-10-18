// A square, dependency-free diagram shared by the demo homepages and examples.
// Three lanes read left to right: inputs & cache → build → publish.
// Every shape is drawn with built-in Typst primitives, so no packages are needed.
#let process-diagram() = {
  set text(font: "Inria Sans", size: 8.8pt, fill: rgb("2b3a4f"))
  set par(leading: 0.5em, justify: false)

  // Palette ------------------------------------------------------------------
  let ink = rgb("1d2939")
  let body-ink = rgb("475467")
  let muted = rgb("7a8699")
  let wire-ink = rgb("98a2b3")
  let blue = rgb("2f6fb3")
  let purple = rgb("7254b0")
  let green = rgb("23845a")
  let amber = rgb("b7791f")
  let orange = rgb("c2410c")
  let red = rgb("b4232c")

  // Geometry helpers (plain numbers are points) -----------------------------
  let at(x, y, content) = place(top + left, dx: x * 1pt, dy: y * 1pt, content)
  let pt(p) = (p.at(0) * 1pt, p.at(1) * 1pt)
  let sign(v) = if v > 0 { 1 } else if v < 0 { -1 } else { 0 }

  // An axis-aligned connector with rounded elbows and an arrowhead at the end.
  let wire(..points, paint: wire-ink, radius: 8) = {
    let ps = points.pos()
    let n = ps.len()
    let dir(a, b) = (sign(b.at(0) - a.at(0)), sign(b.at(1) - a.at(1)))
    let shift(p, d, by) = (p.at(0) + d.at(0) * by, p.at(1) + d.at(1) * by)
    let last = dir(ps.at(n - 2), ps.at(n - 1))
    let tip = ps.at(n - 1)
    let base = shift(tip, last, -6)
    let segments = (curve.move(pt(ps.at(0))),)
    for i in range(1, n - 1) {
      let incoming = dir(ps.at(i - 1), ps.at(i))
      let outgoing = dir(ps.at(i), ps.at(i + 1))
      segments.push(curve.line(pt(shift(ps.at(i), incoming, -radius))))
      segments.push(curve.quad(pt(ps.at(i)), pt(shift(ps.at(i), outgoing, radius))))
    }
    segments.push(curve.line(pt(base)))
    at(0, 0, curve(stroke: (paint: paint, thickness: 1.2pt, cap: "round", join: "round"), ..segments))
    let side = (-last.at(1), last.at(0))
    at(0, 0, curve(
      fill: paint,
      curve.move(pt(tip)),
      curve.line(pt(shift(shift(tip, last, -7), side, 3.6))),
      curve.line(pt(shift(shift(tip, last, -7), side, -3.6))),
      curve.close(),
    ))
  }

  // Small pieces ------------------------------------------------------------
  let chip(label, tint, filled: false) = box(
    inset: (x: 4pt, y: 2pt),
    radius: 3pt,
    fill: if filled { tint } else { tint.lighten(88%) },
    text(size: 6.8pt, weight: "bold", fill: if filled { white } else { tint }, label),
  )
  let badge(number, tint) = box(
    width: 17pt,
    height: 17pt,
    radius: 8.5pt,
    fill: tint,
    align(center + horizon, text(size: 7.5pt, weight: "bold", fill: white, number)),
  )
  let checkpoint(label) = box(
    inset: (x: 4pt, y: 2pt),
    radius: 3pt,
    fill: amber.lighten(88%),
    stroke: 0.5pt + amber.lighten(50%),
    {
      box(baseline: -0.6pt, rotate(45deg, rect(width: 4.4pt, height: 4.4pt, fill: amber)))
      h(3pt)
      text(size: 6.8pt, weight: "bold", fill: amber, label)
    },
  )

  // A process step: numbered badge, title, optional parallel chip, short lines.
  let step(x, y, w, number, title, lines, tint, parallel: false, extra: none, height: 92) = at(x, y, rect(
    width: w * 1pt,
    height: height * 1pt,
    inset: (x: 10pt, top: 9pt, bottom: 8pt),
    radius: 8pt,
    fill: white,
    stroke: 0.9pt + tint.lighten(45%),
    stack(
      dir: ttb,
      spacing: 7pt,
      grid(
        columns: (auto, 1fr),
        column-gutter: 6pt,
        align: horizon,
        badge(number, tint),
        text(size: 10.5pt, weight: "bold", fill: ink, title),
      ),
      text(fill: body-ink, lines.join(linebreak())),
      ..if parallel or extra != none {
        ({
          if parallel { chip([PARALLEL], tint) }
          if parallel and extra != none { h(4pt) }
          if extra != none { extra }
        },)
      },
    ),
  ))

  // A lane: tinted band with a rotated label strip on the left.
  let lane(y, label, tint, h: 124) = {
    at(16, y, rect(width: 628pt, height: h * 1pt, radius: 10pt, fill: tint.lighten(94%), stroke: 0.6pt + tint.lighten(75%)))
    at(16, y, rect(
      width: 24pt,
      height: h * 1pt,
      radius: (left: 10pt),
      fill: tint.lighten(82%),
      align(center + horizon, rotate(-90deg, reflow: true, text(size: 7.2pt, weight: "bold", tracking: 0.8pt, fill: tint, label))),
    ))
  }

  rect(width: 660pt, height: 660pt, inset: 0pt, radius: 12pt, fill: white, stroke: 0.6pt + rgb("dde3ea"), {
    // Header ----------------------------------------------------------------
    at(24, 20, text(size: 20pt, weight: "bold", fill: ink)[Typsite build pipeline])
    at(24, 48, text(size: 9pt, fill: muted)[Reuse what is cached, rebuild what changed, publish only the bytes that differ.])
    at(24, 68, {
      set text(size: 7.6pt, fill: muted)
      chip([PARALLEL], blue)
      h(4pt)
      [Rayon stage]
      h(14pt)
      checkpoint([A / B])
      h(4pt)
      [saved checkpoint]
      h(14pt)
      chip([HIT], green, filled: true)
      h(2pt)
      chip([MISS], orange, filled: true)
      h(4pt)
      [export cache]
    })

    // Lanes ---------------------------------------------------------------
    lane(92, [INPUTS & CACHE], blue)
    lane(250, [BUILD · IF DIRTY], purple)
    lane(408, [PUBLISH], green)

    // Lane 1: discover → export → decide ----------------------------------
    step(48, 108, 156, "01", [Discover], (
      [Lock cache and output],
      [Load config, schemas, themes],
      [Hash every input file],
    ), blue, parallel: true)

    step(228, 108, 158, "02", [Export or reuse], (
      [#chip([HIT], green, filled: true) reuse cached HTML],
      [#chip([MISS], orange, filled: true) run Typst, bounded],
      [Record real dependencies],
    ), blue, parallel: true, extra: checkpoint([A · export cache]))

    // 03: decision diamond.
    at(406, 108, polygon(
      fill: white,
      stroke: 1.1pt + blue.lighten(30%),
      (50pt, 0pt), (100pt, 46pt), (50pt, 92pt), (0pt, 46pt),
    ))
    at(406, 108, box(width: 100pt, height: 92pt, align(center + horizon, stack(
      dir: ttb,
      spacing: 4pt,
      badge("03", blue),
      text(size: 9.2pt, weight: "bold", fill: ink)[Output\ current?],
    ))))

    // No-op terminal.
    at(526, 120, rect(
      width: 110pt,
      height: 68pt,
      radius: 34pt,
      fill: green.lighten(90%),
      stroke: 0.9pt + green.lighten(40%),
      align(center + horizon, stack(
        dir: ttb,
        spacing: 4pt,
        text(size: 10pt, weight: "bold", fill: green)[Done · no-op],
        text(size: 8pt, fill: body-ink)[no writes, no reload],
      )),
    ))

    wire((204, 154), (228, 154))
    wire((386, 154), (406, 154))
    wire((506, 154), (526, 154), paint: green)
    at(507, 139, text(size: 7pt, weight: "bold", fill: green)[YES])

    // NO branch drops into the build lane.
    wire((456, 200), (456, 233), (136, 233), (136, 266), paint: purple)
    at(236, 225, box(
      inset: (x: 6pt, y: 3pt),
      radius: 4pt,
      fill: white,
      stroke: 0.7pt + purple.lighten(40%),
      text(size: 7.4pt, weight: "bold", fill: purple)[NO → something to publish],
    ))

    // Lane 2: parse → resolve → compose ----------------------------------
    step(48, 266, 176, "04", [Parse articles], (
      [Metadata, headings, anchors],
      [Citations and direct embeds],
      [Always fresh, never cached],
    ), purple, parallel: true)

    step(254, 266, 176, "05", [Resolve graph], (
      [Parents, children, backlinks],
      [Removed links stay removed],
      [Embed / parent cycles → error],
    ), purple)

    step(460, 266, 176, "06", [Compose pages], (
      [Metadata → embeds → rewrites],
      [Whole pages from schemas],
      [RSS, sitemap, static assets],
    ), purple, parallel: true)

    wire((224, 312), (254, 312))
    wire((430, 312), (460, 312))
    wire((548, 358), (548, 391), (136, 391), (136, 424))

    // Lane 3: plan → publish → report ------------------------------------
    step(48, 424, 176, "07", [Check output plan], (
      [No traversal, symlinks, clashes],
      [Input changed mid-build?],
      [Retry the export],
    ), green)

    step(254, 424, 176, "08", [Publish changes], (
      [Write only changed bytes],
      [Atomic swap per file],
      [Remove stale owned files],
    ), green, parallel: true, extra: checkpoint([B · ownership journal]))

    step(460, 424, 176, "09", [Report & reload], (
      [Counts and diagnostics],
      [Failures exit nonzero],
      [Preview reloads on change],
    ), green)

    wire((224, 470), (254, 470))
    wire((430, 470), (460, 470))

    // Failure band: kept apart so failures never look like the success path.
    at(16, 556, rect(
      width: 628pt,
      height: 84pt,
      radius: 10pt,
      fill: red.lighten(95%),
      stroke: (paint: red.lighten(55%), thickness: 0.8pt, dash: "dashed"),
    ))
    at(30, 567, text(size: 7.4pt, weight: "bold", tracking: 0.8pt, fill: red)[WHEN A STEP FAILS])
    let failure(x, steps, body) = at(x, 585, box(width: 286pt, {
      text(size: 9.4pt, weight: "bold", fill: red, steps)
      linebreak()
      text(size: 8.4pt, fill: body-ink, body)
    }))
    failure(30, [Steps 01–07 → last good site stays], [
      Nothing is published. Fix and rerun; finished exports are reused.
    ])
    at(330, 585, line(start: (0pt, 0pt), end: (0pt, 42pt), stroke: 0.6pt + red.lighten(65%)))
    failure(342, [Step 08 → rerun to repair], [
      Files swap one at a time, so old and new may mix. Keep
      #raw(".typsite-outputs.json"); the next run repairs safely.
    ])
  })
}
