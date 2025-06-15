
#import "lib.typ": *

#show: schema.with("page")

#let LaTeX = {
  let A = (
    offset: (
      x: -0.33em,
      y: -0.3em,
    ),
    size: 0.7em,
  )
  let T = (
    x_offset: -0.12em    
  )
  let E = (
    x_offset: -0.2em,
    y_offset: 0.23em,
    size: 1em
  )
  let X = (
    x_offset: -0.1em
  )
  [
    L#h(A.offset.x)#text(size: A.size, baseline: A.offset.y)[A]#h(T.x_offset)T#h(E.x_offset)#text(size: E.size, baseline: E.y_offset)[E]#h(X.x_offset)X
  ]
}

#let LaTeX = inline(LaTeX,scale: 200%, auto-filter: true)

#set-metacontent("LaTeX",LaTeX)



#let process = {
  import "@preview/fletcher:0.5.8" as fletcher: diagram, node, edge
  import fletcher.shapes: house, hexagon, circle, rect

  
  let blob(pos, label, tint: white, ..args) = node(
    pos,
    align(center, label),
    width: 28mm,
    fill: tint.lighten(60%),
    stroke: 1pt + tint.darken(20%),
    corner-radius: 5pt,
    ..args,
  )


  let diagram = diagram(
    spacing: 21pt,
    cell-size: (14mm, 10mm),
    edge-stroke: 1pt,
    edge-corner-radius: 7pt,
    mark-scale: 70%,


    blob((0.35, 0.8), [*Input*], shape: house.with(angle: 30deg, dir: bottom), width: 15mm, tint: red),


    for x in (-.2, +.2) {
      edge((0.35, 1.4), (0.35+x, 1.4), (0.35+x, 2), "-|>")
    },

    edge("-|>", [_changed files_], label-side: right, label-pos: 0),

    blob((0.35, 2), [Compile Typst \ to HTML], width: auto, tint: orange),


    for x in (-.2, +.2) {
      edge((0.35, 2), (0.35+x, 2), (0.35+x, 2.9), "-|>")
    },

    edge("-|>"),

    blob((0.35, 3), [Pass HTML], tint: purple),

    for x in (-.2, +.2) {
      edge((0.35+x, 3), (0.35+x, 4), (0.35, 4), "-")
    },
    edge((0.35, 3), (0.35, 4.65), "r", "-|>"),

    blob((1, 4.65), [Dependency Analysis], tint: yellow, shape: hexagon),

    edge((0.35, 3), (1, 3), "--|>"),

    blob((1, 3), [Cache], width: auto, shape: circle, tint: maroon),

    edge(
      (1, 3),
      "d",
      (1, 3.8),
      "-",
      [_articles remain unchanged \ but require an update_],
      label-side: center,
      label-pos: 100%,
      label-size: 0.9em,
    ),

    edge((1, 4), (1, 4.65), "-|>"),

    edge((1, 4.65), (1.65, 4.65), (1.65, 4)),

    for x in (-.2, -.07, +.07, +.2) {
      edge((1.65, 4), (1.65 + x, 4), (1.65 + x, 3), "-|>")
    },


    blob((1.65, 3), [Pass Articles], tint: blue),
    for x in (-.2, -.07, +.07, +.2) {
      edge((1.65, 3), (1.65 + x, 3), (1.65 + x, 2), "-|>")
    },

    blob((1.65, 2), [Compose Pages \ with Schemas], tint: green,width:30mm),


    for x in (-.2, -.07, +.07, +.2) {
      edge((1.65 + x, 2), (1.65 + x, 1.5), (1.65, 1.5), "-")
    },
    
    edge((1.65, 1.5),(1.65, 0.8),"-|>"),

    
    edge((0.35, 0.9),(0.35, 0.3),(1.65, 0.3),(1.65, 0.8),"..|>",[_Sync_]),

    blob((1.65, 0.8), [*Output*], shape: house.with(dir: bottom, angle: 30deg), width: 15mm, tint: red),


    edge((1, 3), (1, 0.8), "--|>"),

    blob((1, 0.8), [*Cache*], shape: house.with(dir: bottom, angle: 30deg), width: 15mm, tint: red),

    edge((0.2, 1.4), (-0.3, 1.4), ".."),
    blob((-0.3, 1.5), [.typ], width: auto, shape: rect, tint: gray),
    edge("..|>"),
    edge((0.15, 2.5), (-0.3, 2.5), ".."),
    blob((-0.3, 2.5), [.html], width: auto, shape: rect, tint: gray),

    edge((0.15, 3.5), (-0.3, 3.5), ".."),
    
    edge((-0.3, 2.5),(-0.3, 5.9),(1, 5.9),"..|>",[_Articles_],label-side:left),

    for y in (+.5,-.5) {
      edge((0.16, 5.9), (0.16, 5.9 + y), (1, 5.9 + y), "..|>")
    },
    
    edge((1, 4.65), (1, 5.4), ".."),
    
    blob((1, 5.4), [(Parents, Children, Cited, Citing)], width: 65mm, shape: rect, tint: gray),
    edge(".."),
    
    blob((1, 5.9), [{ Metadata, Sidebar, Embed ,Rewrite }], width: 65mm, shape: rect, tint: gray),
    edge(".."),
    
    blob((1, 6.4), [[Head,Body]], width: 65mm, shape: rect, tint: gray),
    

    edge((1, 5.9),(2.35, 5.9),(2.35, 2.55),"..|>"),
    for y in (+.5,-.5) {
      edge((1.84, 5.9), (1.84, 5.9 + y), (1, 5.9 + y), "..")
    },
    
    edge((1.85, 3.5), (2.35, 3.5), ".."),
    
    edge((1.85, 2.55), (2.35, 2.55), ".."),
    
    blob((2.35, 2.55), [Pendings], width: auto, shape: rect, tint: gray),
    edge("..|>"),
    blob((2.35, 1.5), [.html], width: auto, shape: rect, tint: gray),
    edge((1.55, 1.5), (2.35, 1.5), ".."),
    
  )
  diagram
}
#title[Typsite 文档]
#date[2025-04-12 02:08]
#author[Glomzzz]

#inline(alignment: center,scale: 37.5%,image("icon.png"))

= 介绍
Typsite 是一个用于构建静态网站的工具，其文章内容由纯 *Typst* 编写，经由 *Typsite* 进行处理后，最终生成一个健全的静态站点。

= 功能

- *Typst* 的常规表达
- 框架: 标题递增、小节模板、侧边栏、页脚
- 富文本：段落、小节、引用、代码块、数学公式、注脚、页面嵌入
- 支持现代 Web 技术规范，如 HTML5、CSS3 和 JavaScript（ES6+）
- 增量编译, 实时预览


= #embed("./typst.typ", sidebar: "only-title")

= 安装

- 通过 #link("https://github.com/Glomzzz/typsite/releases/latest")[Release 页面]下载二进制文件
  - 请确保你已安装了*0.13+*的typst
- 通过 Nix & Flakes 构建
  - 请确保你已经开启了 `experimental-features = nix-command flakes`

```shell
git clone https://github.com/Glomzzz/typsite.git

cd typsite
nix build .
```

= 初始化

通过`typsite init`, 可以在当前文件夹初始化 Typsite.

```
.
├── root     ---  typst 根目录
│   ├── index.typ    --- 文章
│   └── lib.typ      --- typsite 库文件
├── .typsite --- typsite 配置目录
│   ├── assets       ---  资源目录 (会同步到输出目录) 
│   ├── components   ---  组件模板
│   ├── themes       ---  代码高亮
│   ├── syntaxes     ---  代码语法
│   ├── rewrite      ---  重写器模板
│   ├── schemas      ---  页面模板
│   └── options.toml ---  项目配置
├── .cache   ---  缓存目录
└── publish  ---  输出目录
```

= 写作

#import "@preview/frame-it:1.2.0": *

#let (example, feature, variant, syntax) = frames(
  feature: ("Feature",),
  // For each frame kind, you have to provide its supplement title to be displayed
  variant: ("Variant",),
  // You can provide a color or leave it out and it will be generated
  example: ("Example", gray),
  // You can add as many as you want
  syntax: ("Syntax",),
)
// This is necessary. Don't forget this!
#show: frame-style(styles.boxy)

Typsite 兼容绝大部分 typst 的原生表达:
#inline(scale: 150%, alignment: center)[
  #table(
  columns: (auto, auto,auto,auto),
  stroke: 0.4pt,
  fill: white,
  align: center, // Align first column left, rest center
  [*Element*], [*Support Status*],[*Element*], [*Support Status*],
  [`heading`], [#text(fill: blue.lighten(50%),[*t*])],
  [`par`], [#text(fill: blue.lighten(50%),[*t*])],
  [`text`], [#text(fill: blue.lighten(50%),[*t*])],
  [`strong`], [#text(fill: blue.lighten(50%),[*t*])],
  [`list`], [#text(fill: blue.lighten(50%),[*t*])],
  [`emph`], [✅],
  [`overline`], [✅],
  [`underline`], [✅],
  [`super`], [✅],
  [`sub`], [✅],
  [`raw`], [✅],
  [`link`], [✅],
  [`label`], [✅],
  [`ref`], [✅],
  [`footnote`], [✅],
  [`math.equation`], [✅],
  [`highlight`], [✅],
  [`text with color`], [✅],
  [`align`#super[1]], [inline#super[2]],
  [`strike`], [inline],
  [`table`], [inline],
  [`terms`], [inline],
  [`figure`], [inline],
)

]

#footnote[对于align, 请单独使用库中的 `text-align` 函数] <align>
#footnote[对于复杂样式的元素, 请使用 `inline` 函数进行 *svg* 内联]

= 命令行

```
用法: typsite <COMMAND>

命令:
  init     在指定目录中初始化一个新的 typsite 项目
  compile  编译或监听项目，指定输入和输出目录 [别名: c]
  clean    清除缓存和输出目录
  syntect  查看代码高亮&语法支持列表
  help     打印此消息或指定子命令的帮助信息

选项:
  -h, --help     打印帮助
  -V, --version  打印版本信息
```

== init

```
在指定目录中初始化一个新的 typsite 项目

用法: typsite init [OPTIONS]

选项:
  -d, --dir <DIR>  项目根目录 [默认: ./]
  -h, --help       打印帮助
```

== compile

```
编译或监听项目，指定输入和输出目录

用法: typsite compile [OPTIONS]

选项:
      --host <HOST>      服务主机 [默认: localhost]
      --port <PORT>      服务端口, 如果非0则进入watch-mode [默认: 0]
      --config <CONFIG>  项目配置路径 [默认: ./.typsite]
      --cache <CACHE>    缓存目录 [默认: ./.cache]
  -i, --input <INPUT>    Typst 根目录，存放 typst 文件的位置 [默认: ./root] [别名: --i]
  -o, --output <OUTPUT>  输出目录 [默认: ./publish] [别名: --o]
      --no-pretty-url    
      --no-short-slug    
  -h, --help             打印帮助
```

== clean

```
清除缓存和输出目录

用法: typsite clean [OPTIONS]

选项:
  -o, --output <OUTPUT>  输出目录 [默认: ./publish]
  -c, --cache <CACHE>    缓存目录，用于存储原始 typst_html_export 内容 [默认: ./.cache]
  -h, --help             打印帮助
```

== syntect

```
查看代码高亮&语法支持列表
用法: typsite syntect [OPTIONS]

选项:
      --config <CONFIG>  项目配置路径 [默认: ./.typsite]
  -h, --help             打印帮助
```

= 架构 & 流程

#inline(scale: 200%,fill: color.white,alignment: center)[#process]

= 配置

你可以在这里看到所有的默认配置: 

基于这些配置, 你可以完全地自定义你的整个站点.

- schema: 页面模板, 负责处理页面框架
- components / rewrites: 组件/重写器, 组成页面内容
- assets: 资源文件目录, 会在compile时自动同步到输出目录
- themes: 代码高亮文件
- syntaxes: 代码语法文件
 
= #embed("./article.typ", sidebar: "only_title", open: false)
#text-align(center)[要不先来看看#cite-title("./example.typ")]

