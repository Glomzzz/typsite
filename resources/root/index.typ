
#import "/lib/lib.typ": *

#show: schema.with("page")


#import "/lib/process.typ": process-diagram

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
#rss(true)
#date[2025-04-12 02:08]
#author[Glomzzz]

#metacontent("process")[
  #html.div(class: "process-diagram", role: "img", aria-label: "Typsite build pipeline: export cache, no-op branch, graph resolution, page and feed composition, atomic publication, and failure recovery")[
    #inline(process-diagram())
  ]
]

#metacontent("icon")[
  #context {
    if target() == "html" {
      html.align(center, html.img(
        src: "/icon.svg",
        alt: "Typsite",
        width: "192",
        height: "192",
        style: "width: 192px; max-width: 100%; height: auto;",
      )[])
    } else {
      align(center, image("icon.svg", width: 144pt))
    }
  }
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
  -j, --jobs <JOBS>      并发导出的文章数 [默认: 可用 CPU/Rayon 预算]
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

== 并行构建

Typsite 使用 Rayon 并行计算独立文件的指纹、校验导出缓存、解析文章 HTML 和渲染页面。Typst 导出使用独立的有界 Rayon 池：默认以可用 CPU 预算（同时参考 Rayon 池大小）为并发上限。每个 Typst 子进程再通过自身的 `--jobs` 分得一部分 CPU 预算，避免每个子进程都启动整台机器数量的工作线程。

```sh
typsite compile --jobs 4
# 内存受限或排查导出问题时，每次只运行一个文章子进程。
typsite compile --jobs 1
```

`-j` 是 `--jobs` 的别名，参数必须是正整数。它控制的是*同时导出的文章数*，不是全部 Rayon 工作；文件指纹、解析和渲染的线程可另用 `RAYON_NUM_THREADS` 调整。显式设置较大的 `--jobs` 可能增加内存使用，但实际并发数不会超过待导出的文章数。真正无变化的构建不会创建导出线程池。

包在导出前统一准备。各工作线程生成独立产物，全部结束后再按稳定的源文件顺序汇总。关系解析、依赖有序的元数据、缓存/归属检查点和过期删除仍保持有序。独立输出文件并行原子写入并同步数据，父目录按批次同步。已验证的无变化构建跳过配置解析，合法高亮资源按内容指纹缓存，共享依赖每批导出校验一次。只改变 `--jobs` 不会使缓存失效，也不会改变站点字节。

= RSS 订阅与站点地图

*RSS 需要主动开启。* 文章标记 `rss` 默认是 `false`，内置配置的 `base_url` 留空。生成订阅需要*同时*填写站点地址，并在集合页通过 `#rss(true)` 选择文章。不要把全局文章默认值改为 true。

== 1. 开启订阅文件生成

在 `.typsite/options.toml` 中填写站点公开地址，即可生成 RSS 2.0 和站点地图：

```toml
[site]
base_url = "https://example.org"
title = "我的站点"
description = "笔记与文章"
rss_path = "/rss.xml"
sitemap_path = "/sitemap.xml"
rss_limit = 20
rss_content = "summary" # 可选 "summary" 或 "full"
```

请修改已有的 `[site]`，不要重复添加。把示例地址换成自己的公开地址后，运行 `typsite compile`。文件写入输出目录；每个生成页面的 HTML head 中都会加入 RSS 自动发现链接，读者可订阅 `https://example.org/rss.xml`。

== 2. 通过集合页选择文章

文章元数据新增布尔选项 `rss`，默认是 `false`。在集合页写上 `#rss(true)`，并直接嵌入要订阅的文章：

```typ
#import "/lib/lib.typ": *
#show: schema.with("page")
#title[我的订阅集合]
#rss(true)

= #embed("./first-post.typ")
= #embed("./second-post.typ")
```

只有*直接嵌入的文章*会成为订阅条目。标记为 `rss: true` 的集合页自身不进入 RSS，即使被其他集合页嵌入也不例外。标记不会被嵌入文章继承。嵌套嵌入不会自动成为独立条目，但全文模式仍会保留所选文章里的嵌入内容。引用、父级元数据和仅声明的 child 图关系不参与选择。多个集合页选出的文章合并去重，每篇只出现一次。

`#rss(false)` 将页面恢复为普通文章；如果仍被某个已标记的集合页直接嵌入，它仍可进入 RSS。没有标记集合页或没有可选直接嵌入时，已启用的订阅文件为空。日期排序和 `rss_limit` 在*选出条目之后*应用。站点地图仍包含所有编译文章。

演示站点的两个首页都是集合页，各自直接嵌入文章指南和 Typst 简介，因此 RSS 共*四个条目*，而不是全部页面。旧站点库可从 `@local/typsite:0.1.0` 导入 `rss`，或合并新版 `root/lib/lib.typ` 的导入列表。

== 3. 在演示站点中试用

填写 `base_url` 后，在初始化的演示站点目录运行：

```sh
typsite compile
typsite compile --port 8080
```

检查 `publish/rss.xml`，或在预览运行时打开 `http://localhost:8080/rss.xml`。部署输出后，用公开地址（例如 `https://example.org/rss.xml`）订阅；预览地址只用于本地检查。

没有文件时，检查 `base_url`、`rss_path`、`rss_limit`；文件为空时，检查是否存在带有可选直接嵌入的标记集合页。保留 `[default_metadata.options].rss = false`；`#rss(true)` 写在集合页，而不是准备发布的文章上。完整的升级操作见 #cite("./migrate-to-018.typ")[0.1.8 RSS 迁移步骤]。

== 条目内容与日期

- `summary` 输出纯文本摘要，依次取非空的 `description`、`summary`、`abstract` 元数据。没有这些元数据时，不会自动截取正文。
- `full` 还会在 `content:encoded` 中包含组合后的文章 HTML，包括嵌入文章，但不包含页面模板、导航、侧边栏或引用与反向链接页脚。普通链接和图片地址会转为绝对 URL。脚本会被移除；RSS 阅读器可能无法像网页一样显示 SVG、MathML、交互元素或站点专用 CSS。
- 标题优先使用 `page-title`，其次为 `title`。排序时间依次取 `updated`、`lastmod`、`date`，由新到旧；时间相同时按 URL 排序。`pubDate` 使用发表 `date`，缺失时回退到修改时间。
- 日期支持 `YYYY-MM-DD`、`YYYY-MM-DD HH:MM[:SS]`（按 UTC）、RFC 3339 和 RFC 2822。无效或未知日期会被省略，不会写出不合法的 RSS 日期。
- `base_url` 留空会关闭两种输出。`rss_path` 留空或 `rss_limit = 0` 只关闭 RSS；`sitemap_path` 留空只关闭站点地图。关闭或改名后，旧生成文件通过归属记录清理。
- 修改订阅配置只重新组合输出，不会让未变更文章重新经过 Typst 导出。`--no-pretty-url` 会反映在文章链接中。

文章可以使用现有辅助函数提供元数据：

```typ
#title[一篇文章]
#date[2026-03-02 18:25]
#metacontent("summary")[写给订阅阅读器的简短介绍。]
#metacontent("updated")[2026-03-03T09:00:00Z]
```

= 架构 & 流程

#get-metacontent("process")

方形图按编号蛇形阅读：第一行从左到右，第二行从右到左，第三行再从左到右。HIT 复用原始 HTML，MISS 重新调用 Typst；发布状态未变时在步骤 03 直接结束。只有需要发布更新时，才继续解析、关系解析和完整页面组合。两个独立检查点分别保存导出结果与发布状态，底部红色区域说明失败恢复。小屏可在图内横向滚动查看细节。

Rayon 并行执行独立文件指纹、文章解析、页面渲染和有界 Typst 导出；包准备与发布检查点仍保持有序。
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
