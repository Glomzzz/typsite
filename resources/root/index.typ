
#import "/lib/lib.typ": *

#show: schema.with("page")


#let _process = {
  // Built-in drawing keeps the demo self-contained: no preview packages.
  set text(font: "Inria Sans", size: 10pt)
  let stage(body, tint) = rect(
    width: 100%,
    inset: 6pt,
    radius: 4pt,
    fill: tint.lighten(90%),
    stroke: 0.6pt + tint,
    align(center, body),
  )
  let arrow = [↓]
  grid(
    columns: (76mm,),
    align: center,
    row-gutter: 4pt,
    stage([Discover project inputs], blue),
    arrow,
    stage([Export changed Typst or reuse raw HTML\ + dependency hashes], orange),
    arrow,
    stage([Parse current article facts], purple),
    arrow,
    stage([Resolve and validate current relationships], maroon),
    arrow,
    stage([Recompose all pages and feeds], blue),
    arrow,
    stage([Publish changed bytes atomically per file\ + output ownership journal], green),
    arrow,
    stage([Record publication and report results], teal),
  )
}

#let _typst-support = table(
  columns: (auto, auto, auto, auto),
  stroke: 0.4pt,
  fill: white,
  align: center,
  [*Element*], [*Support Status*], [*Element*], [*Support Status*],
  [`heading`], [#text(fill: blue.lighten(50%), [*t*])], [`par`], [#text(fill: blue.lighten(50%), [*t*])],
  [`text`], [#text(fill: blue.lighten(50%), [*t*])], [`strong`], [#text(fill: blue.lighten(50%), [*t*])],
  [`list`], [#text(fill: blue.lighten(50%), [*t*])], [`emph`], [✅],
  [`overline`], [✅], [`underline`], [✅],
  [`super`], [✅], [`sub`], [✅],
  [`raw`], [✅], [`link`], [✅],
  [`label`], [✅], [`ref`], [✅],
  [`footnote`], [✅], [`math.equation`], [✅],
  [`highlight`], [✅], [`text with color`], [✅],
  [`align`#footnote(<align>)], [inline#footnote(<svg-inline>)], [`strike`], [inline],
  [`table`], [inline], [`terms`], [inline],
  [`figure`], [inline],
)

#title[Typsite 文档]
#date[2025-04-12 02:08]
#author[Glomzzz]

#metacontent("process")[
  #html.align(center, inline(block(fill: color.white, _process)))
]

#metacontent("icon")[
  #html.align(center, inline(scale: 37.5%, image("icon.png")))
]

#set text(font: "Inria Sans") // From .typsite/assets/font

#metacontent("typst-support")[
  #html.align(center)[
    #inline(scale: 150%, _typst-support)
  ]
]

#get-metacontent("icon")



#html.align(center)[ #cite-title("migrate-to-018.typ") ]
#html.align(center)[ #cite-title("migrate-to-017.typ") ]
#html.align(center)[ #cite-title("migrate-to-016.typ") ]

= 介绍
Typsite 是一个用于构建静态网站的工具，其文章内容由纯 *Typst* 编写，经由 *Typsite* 进行处理后，最终生成一个健全的静态站点。

= 功能

- *Typst* 的常规表达
- 框架: 标题递增、小节模板、侧边栏、页脚
- 富文本：段落、小节、引用、代码块、数学公式、注脚、页面嵌入
- 将 Typst math 转换为 Mathml （自动检测math-font)
- 支持现代 Web 技术规范，如 HTML5、CSS3 和 JavaScript（ES6+）
- 增量编译, 实时预览


= #embed("./typst.typ", sidebar: "only-title")

= 安装

- 通过 #link("https://github.com/Glomzzz/typsite/releases/latest")[Release 页面]下载二进制文件
  - 请使用 *Typst 最新稳定版*（当前已测试 *0.15.1*）
  - 从源码构建需要 *Rust 1.89 或更新版本*
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
│   └── lib/         --- typsite 库文件，入口为 lib/lib.typ
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

Typsite 兼容绝大部分 typst 的原生表达:

#get-metacontent("typst-support")

#footnote[对于align, 请单独使用库中的 `html.align` 函数] <align>
#footnote[对于复杂样式的元素, 请使用 `inline` 函数进行 *svg* 内联] <svg-inline>

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
  -p, --packages <PACKAGES>  本地包目录，typsite会将在此目录的包自动安装到@local，若在watch-mode则会自动同步，如果为空或未找到目录则跳过 [默认: ""] [别名: --p]
  -t, --typst <TYPST>    Typst 可执行程序路径 [默认: typst]
      --no-pretty-url
      --no-short-slug
  -h, --help             打印帮助
```

== clean

```
清除缓存和输出目录

用法: typsite clean [OPTIONS]

选项:
      --input <INPUT>    受保护的 Typst 源文件目录 [默认: ./root]
      --config <CONFIG>  受保护的配置目录 [默认: ./.typsite]
      --force            检查路径后允许清理没有归属标记的旧目录
  -o, --output <OUTPUT>  输出目录 [默认: ./publish]
  -c, --cache <CACHE>    缓存目录，用于存储原始 Typst HTML [默认: ./.cache]
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

#get-metacontent("process")

构建缓存保存原始 Typst HTML 和实际依赖的指纹，不保存旧关系图或渲染片段。
需要更新时重新组合所有页面，但只写入内容发生变化的文件；无变化的构建不写输出，也不刷新预览。
发布按单个文件原子替换，不是整个站点的事务；失败后修复问题并重新运行即可。
升级与输出归属说明见 #cite-title("migrate-to-018.typ")。

= 配置

你可以在这里看到所有的默认配置:

基于这些配置, 你可以完全地自定义你的整个站点.

- schema: 页面模板, 负责处理页面框架
- components / rewrites: 组件/重写器, 组成页面内容
- assets: 资源文件目录, 会在compile时自动同步到输出目录
- themes: 代码高亮文件
- syntaxes: 代码语法文件

= #embed("./article.typ", sidebar: "only_title", open: false)
#html.align(center)[要不先来看看#cite-title("./example.typ")]
