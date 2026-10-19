#import "/lib/lib.typ": *

#show: schema.with("page")


#title[内容示例]
#date[2025-06-05 07:12]
#author[Glomzzz]
#parent("/index.typ")

== 好看的

这是一段普通的文本.

这是 #LaTeX

#html.align(center)[

  #html.text(size: 52pt, weight: "bold", fill: rgb("#22D3EE"))[Typst]
  \
  \
  #html.text(size: 38pt, fill: rgb("#22D3EE"))[🔥*已经崛起了!*🔥 <rise-up> ]

  \
  \

  #html.text(size: 22pt, style: "italic", fill: red)[🚀_这TeX人没收到通知吗？_🚀]
  \
  \
]

\

#html.align(center)[
  #html.text(size: 52pt)[#LaTeX |-> #html.text(fill: rgb("#22D3EE"))[Typst]]
]

\

#html.align(center)[
  #html.text(size: 40pt)[#underline[我的]#highlight(fill: green.lighten(50%))[大树人]，#overline[没了]！#footnote(<np>)]
]

\

带着好看字体的 Blockquote ：

#block-quote[
  #html.text(size: 85%, style: "normal", frame: html.div)[
    Typst 是一种现代化的排版系统，类似于 LaTeX，但设计更为简洁、易学，它主要用于创建学术论文、书籍、报告等需要精美排版的文档。

    你可以在这里查看其官方英文文档：#link("https://typst.app/docs/")[Typst Document]; \
    对于Typst的中文教程，我强烈推荐：#link("https://typst-doc-cn.github.io/tutorial/introduction.html")[Typst 蓝书]#note[\[天呐，这位编者非常清楚地知道自己是在阐述一套*本体论*!\]].
  ]
]

\

数学公式（MathML）：

$
  ker tau & = {[x]_U in V slash U | [x]_W = [0]_W} \
          & = {[x]_U in V slash U | x in W}        \
$

注脚：

#footnote[自然先知的铁树树人, 在7.39b 游戏性版本更新中, 也许永远地离开了我们....] <np>

== 好玩的

点@amazing[我]能跳转到神奇的地方.

点@rise-up[我]能跳转到 #html.text(fill: red)[*崛起*]!

#details([点我可以看一些好东西])[哈哈, #link("https://www.bilibili.com/video/BV1yaSHYNEen")[#html.text(fill: yellow.darken(15%))[_300颗够吗_]]], 还有#details([我])[
  #link("https://www.bilibili.com/video/BV1hN411a7Ky")[*永远*没有人看完这把刀塔还能#html.text(fill: purple.darken(15%))[绷得住]，记住，是#html.text(fill: red)[*永远*]]
]

== 好听的

Another One Bites the Dust#footnote(<dust>)

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

#footnote[ #link("https://music.apple.com/us/song/another-one-bites-the-dust/1440650719")[来听!] ] <dust>

== 神奇的地方 <amazing>

引用: #cite("./typst.typ")[我能自定义引用段的内容] or 我也能直接用引用文章的标题: #cite-title("./typst.typ")

我还能嵌入页面!

#html.text(size: 30pt)[⬇️] 我还能直接把嵌入的内容当作某一个特定heading-level的section来用!
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

=== Typsite 流程图


#get-metacontent("process", from: "/index.typ")

