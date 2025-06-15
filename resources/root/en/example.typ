#import "../lib.typ": *

#show: schema.with("page")

#import "../index.typ" : process


#title[Content Example]
#date[2025-06-05 07:12]
#author[Glomzzz]
#parent("/en/index.typ")

== Beautiful Things

This is a regular paragraph of text.

This is #metacontent("LaTeX",from: "/index.typ")

#text-align(center)[

  #html-text(size: 52pt, weight: "bold", fill: rgb("#22D3EE"))[Typst]
  
  
  #html-text(size: 38pt, fill: rgb("#22D3EE"))[🔥*has risen!*🔥 <rise-up> ] 

  
  

  #html-text(size: 22pt, style: "italic", fill: red)[🚀_Did the TeX folks miss the memo?_🚀]
  
  
]

#text-align(center)[
  #html-text(size: 40pt)[#underline[My] #highlight(fill: green.lighten(50%))[Treant] is #overline[gone]! #footnote(<np>)]
]

$
  ker tau & = {[x]_U in V slash U | [x]_W = [0]_W} 
  & = {[x]_U in V slash U | x in W} 
$

#footnote[The Iron Tree Treant of the Nature Prophet may have left us forever... in the 7.39b gameplay update.] <np>

== Fun Things

Click @amazing[me] to jump to a magical place.

Click @rise-up[me] to jump to #html-text(fill: red)[*Rise Up*!]

#details([Click me to see some good stuff])[Haha, #link("https://www.youtube.com/watch?v=dQw4w9WgXcQ")[#html-text(fill: yellow.darken(15%))[_NEVER GONNA GIVE U UP_]]]

== Nice Music

Another One Bites the Dust #footnote(<dust>)

#text-align(center)[
  #html.elem(
    "iframe",
    attrs: (
      allow: "autoplay *; encrypted-media *; fullscreen *; clipboard-write",
      frameborder: "0",
      height: "175",
      style: "width:100%;max-width:660px;overflow:hidden;border-radius:10px;",
      sandbox: "allow-forms allow-popups allow-same-origin allow-scripts allow-storage-access-by-user-activation allow-top-navigation-by-user-activation",
      src: "https://embed.music.apple.com/my/song/time-flows-ever-onward/1749333759",
    ),
  )

  #html.elem(
    "iframe",
    attrs: (
      style: "border-radius:12px",
      src: "https://open.spotify.com/embed/track/5QspiGbL0BiWfBdm3iSJal?utm_source=generator",
      width: "100%",
      height: "352",
      frameBorder: "0",
      allowfullscreen: "",
      allow: "autoplay; clipboard-write; encrypted-media; fullscreen; picture-in-picture",
      loading: "lazy",
    ),
  )
]

#footnote[ #link("https://music.apple.com/us/song/another-one-bites-the-dust/1440650719")[Listen here!] ] <dust>

== Magical Place <amazing>

Citation: #cite("./typst.typ")[I can customize citation block content] or I can also just cite the article title directly: #cite-title("./typst.typ")

I can even embed a whole page!

#html-text(size: 30pt)[⬇️] I can also treat embedded content as a section with a specific heading level!
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

#import "./index.typ": process

#inline(scale: 200%, fill: color.white, alignment: center)[#process]


