#import "/lib/lib.typ": *

#show: schema.with("page",
  head: [
    #html.tag(
      "link",
      rel: "stylesheet",
      href: "https://fonts.googleapis.com/css2?family=LXGW+WenKai+TC&amp;display=swap",
    )[]
  ],)



#title[Content Example]
#date[2025-06-05 07:12]
#author[Glomzzz]
#parent("/en/index.typ")

== Beautiful Things

This is a regular paragraph of text.

This is #LaTeX


#html.align(center)[

  #html.text(size: 52pt, weight: "bold", fill: rgb("#22D3EE"))[Typst]


  #html.text(size: 38pt, fill: rgb("#22D3EE"))[🔥*has risen!*🔥 <rise-up> ]




  #html.text(size: 22pt, style: "italic", fill: red)[🚀_Did the TeX folks miss the memo?_🚀]


]

\

#html.align(center)[
  #html.text(size: 52pt)[#LaTeX |-> #html.text(fill: rgb("#22D3EE"))[Typst]]
]

\

#html.align(center)[
  #html.text(
    size: 40pt,
  )[#underline[My] #highlight(fill: green.lighten(50%))[Treant] is #overline[gone]! #footnote(<np>)]
]

\

Blockquote with a nice font:

#block-quote[
  // Check this article's head, where we imported the LXGW WenKai TC font
  #html.text(size: 85%, font: "LXGW WenKai TC", style: "normal", frame: html.div)[

    Typst is a modern typesetting system, similar to LaTeX, but designed to be more concise and easier to learn. It is primarily used to create beautifully typeset documents such as academic papers, books, and reports.

    You can find the official English documentation here: #link("https://typst.app/docs/")[Typst Document]; \
    For a Chinese tutorial on Typst, I highly recommend: #link("https://typst-doc-cn.github.io/tutorial/introduction.html")[The Typst Blue Book] \
    #note[\[Oh my, the author is *clearly aware* that they're explaining an *ontology*!\]].
  ]
]

\

Math time!

$
  ker tau & = {[x]_U in V slash U | [x]_W = [0]_W} \
  & = {[x]_U in V slash U | x in W}
$

Section footnote:

#footnote[The Iron Tree Treant of the Nature Prophet may have left us forever... in the 7.39b gameplay update.] <np>

== Fun Things

Click @amazing[me] to jump to a magical place.

Click @rise-up[me] to jump to #html.text(fill: red)[*Rise Up*!]

#details([Click me to see some good stuff])[Haha, #link("https://www.youtube.com/watch?v=dQw4w9WgXcQ")[#html.text(fill: yellow.darken(15%))[_NEVER GONNA GIVE U UP_]]]

== Nice Music

Another One Bites the Dust #footnote(<dust>)

#html.align(center)[
  #html.tag(
    "iframe",
    allow: "autoplay *; encrypted-media *; fullscreen *; clipboard-write",
    frameborder: "0",
    height: "175",
    style: "width:100%;max-width:660px;overflow:hidden;border-radius:10px;",
    sandbox: "allow-forms allow-popups allow-same-origin allow-scripts allow-storage-access-by-user-activation allow-top-navigation-by-user-activation",
    src: "https://embed.music.apple.com/my/song/time-flows-ever-onward/1749333759",
  )[]

  #html.tag(
    "iframe",
    style: "border-radius:12px",
    src: "https://open.spotify.com/embed/track/5QspiGbL0BiWfBdm3iSJal?utm_source=generator",
    width: "100%",
    height: "352",
    frameBorder: "0",
    allowfullscreen: "",
    allow: "autoplay; clipboard-write; encrypted-media; fullscreen; picture-in-picture",
    loading: "lazy",
  )[]
]

#footnote[ #link("https://music.apple.com/us/song/another-one-bites-the-dust/1440650719")[Listen here!] ] <dust>

== Magical Place <amazing>

Citation: #cite("./typst.typ")[I can customize citation block content] or I can also just cite the article title directly: #cite-title("./typst.typ")

I can even embed a whole page!

#html.text(size: 30pt)[⬇️] I can also treat embedded content as a section with a specific heading level!
=== #embed("./typst.typ", open: false, sidebar: "only-title", show-metadata: true)


=== RUUUST
```rust
fn main() {
    let f: fn(&'static str) -> usize = |s| unsafe {
        *s.as_ptr().offset(1) as usize & 0xFF
    };
    println!("{}", (0..5).map(|i| f("hello") ^ i).fold(0, |a, b| a ^ b));
}
```

=== Typsite Flowchart

#get-metacontent("process", from: "/index.typ")


