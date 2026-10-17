#import "/lib/lib.typ": *

#show: schema.with("page")

#let version = html.text(fill: rgb("#22D3EE"))[0.1.8]

#title[迁移到 Typsite #version]
#page-title[迁移到 Typsite 0.1.8]
#date[2026-10-18]
#author[Glomzzz]
#parent("index.typ")
#metacontent("summary")[升级辅助函数、配置 RSS，并安全迁移到 Typsite 0.1.8 的可恢复构建引擎。]

Typsite #version 更换了构建引擎，修复了增量构建残留旧输出的问题，并支持 Typst 最新稳定版。已有的合法文章、模板、配置项和命令行选项继续受支持。本文说明从 0.1.7 升级时需要检查的变化。

从更早版本升级时，请先阅读 #cite-title("migrate-to-017.typ")。

= 升级清单

+ 备份文章、`root/lib` 和 `.typsite`，尤其是自己修改过的模板和辅助函数。
+ 安装 Typsite *0.1.8* 和 *Typst 最新稳定版*。本次已测试 Typst *0.15.1*，不保证兼容旧版 Typst。
+ 在另一个目录生成参考项目，不要在现有站点上直接初始化：

  ```sh
  typsite init --dir ../typsite-018-reference
  ```

+ 对照参考项目的 `root/lib` 和 `.typsite` 合并更新，保留自己的定制。
+ 执行平常使用的编译命令。旧内部缓存会自动按缓存未命中处理，通常不需要先手动清理。

  ```sh
  typsite --version
  typst --version
  typsite compile
  ```

+ 部署前同时检查生成的页面和命令退出状态。构建失败现在会返回非零退出码。

= 站点库与 Typst

== 更新 inline-math

请合并参考项目中的 `root/lib/site.typ`。`inline-math` 现在在上下文中求值，查询脚注标签并传给 SVG 渲染器，支持行内和块级公式回退：

```typ
#inline-math($a/b$)
#inline-math($sum_(i=1)^n i$, block: true)
```

已有调用不需要修改函数签名。Typsite 会自动向 Typst 传入 HTML 功能标志和 `html-frames` 输入参数。

== 不要修改内置包的版本号

*Typsite 可执行程序*的版本是 0.1.8，内置 *Typst 包*继续使用原来的标识：

```typ
#import "@local/typsite:0.1.0": *
```

不要把导入改为 `@local/typsite:0.1.8`。需要导出时，可执行程序会先安装随附的包。

== 编译内置演示站点

初始化的演示站点包含本文、模板、字体和 SVG 示例，构建时不需要下载 preview 包。在参考项目目录中运行：

```sh
typsite compile
typsite compile --port 8080
```

第二条命令启动预览，可访问 `http://localhost:8080/` 或 `/en/`。内容示例中的外部音乐播放器和网页字体仍需要浏览器联网，但不是构建依赖。

= 缓存与输出归属

新缓存保存 Typst 原始 HTML，以及 Typst 实际读取文件的指纹。导入或引用的文件变化时，会重新导出相应文章。旧缓存中的关系图和渲染片段不再恢复。

需要更新输出时，Typsite 会重新建立当前关系图，并重新组合所有页面，但只写入字节发生变化的文件。删除引用后，对应的旧反向链接会消失；删除或重命名文章后，旧的受管理输出也会移除。

输出目录中新增独立的归属记录：

```text
publish/.typsite-outputs.json
```

*请保留这个文件。* 它不是可随意丢弃的缓存。Typsite 据此删除过期生成文件，同时保留无关文件。如果记录损坏，构建会拒绝猜测哪些文件可以删除；修复前请先备份。手动删除它，意味着放弃对之前输出文件的归属记录。

升级前生成、尚未被记录的文件不会被自动接管或删除。如果已经删除的旧页面仍留在输出目录中，请手动移除已确认过期的文件，或检查输出目录内容后显式执行清理。

== 更安全的 clean

`typsite clean` 会在删除前检查两个目标目录、取得构建锁，并拒绝清理与源文件或配置重叠的路径。没有归属标记的非空旧目录，需要明确允许：

```sh
# 先检查这两个目录；clean 会清空它们的内容。
typsite clean --cache ./.cache --output ./publish --force
```

源文件或配置目录不是默认位置时，也要传入，以便保护这些目录：

```sh
typsite clean --input ./content --config ./config \
  --cache ./build-cache --output ./public --force
```

`--force` 只允许清理没有标记的旧目录，*不会*绕过路径安全检查或正在使用的构建锁。清理后可能保留锁文件；构建或预览运行时，不要删除 `.typsite.lock`。

= 更严格的校验

过去被忽略的错误，现在可能导致构建失败。请修复或移除无效资源，不要再依赖静默跳过：

- 页面模板、组件和重写器 HTML 必须包含完整的 `<head>...</head>` 与 `<body>...</body>`。有意留空时，可使用 `<head/>` 或 `<body/>`。
- 损坏的主题、语法定义、语法元数据和重写器，即使没有用到，也会报错；重复的资源标识会被拒绝。
- 页面不能显式把自己设为父页面，也不能嵌入自己。父级继承、嵌入或元数据依赖形成循环时会报错，不再挂起；普通的相互引用和自引用仍然合法。
- 源文件、配置、缓存和输出目录不能相互重叠；本地包源也不能与缓存或输出目录重叠。输出冲突、仅大小写不同的路径冲突、越界路径和受管理输出中的符号链接会被拒绝。目录名中的空格和方括号按字面处理。
- `.typsite.lock` 与 `.typsite-outputs.json` 是保留的输出名称，包括大小写变体，以及这些名称之下的路径。

= 包目录与可复现构建

`--packages` 仍指向本地包的*源文件目录*，不是 Typst 包命名空间。安装前会拒绝无效的包名、版本和重复标识。

需要隔离时，可设置 `TYPST_PACKAGE_PATH`，包会安装到它下面的 `local/<name>/<version>`。Typsite 同时支持 `TYPST_PACKAGE_CACHE_PATH`。例如在 POSIX shell 中：

```sh
export TYPST_PACKAGE_PATH="$PWD/.typst-packages"
export TYPST_PACKAGE_CACHE_PATH="$PWD/.typst-package-cache"
typsite compile
```

这些目录应与源文件、配置、构建缓存和输出目录分开。没有指定路径时，仍保留默认的共享包安装行为。需要可复现导出时，请使用项目字体和固定的 `SOURCE_DATE_EPOCH`；系统字体或未声明的外部输入发生变化后，可能需要丢弃构建缓存。

= RSS 订阅配置

RSS 仍需手动开启：把 `[site].base_url` 设为站点公开的 HTTP(S) 地址。默认的 `rss_path = "/rss.xml"` 和 `rss_limit = 20` 不变。新增 `rss_content = "summary"` 保留摘要和链接；选择 `"full"` 时，还会输出组合后的文章 HTML。开启后，每个生成页面都会加入 RSS 自动发现链接。完整示例见 #cite("index.typ")[首页的 RSS 配置章节]。

RSS 不再包含所有编译文章。请在集合页加入 `#rss(true)`，仅选出它直接嵌入的文章，标记页自身始终排除。多个集合的条目合并去重；布尔选项默认是 false，不会继承。嵌套嵌入、引用或父级关系不选出独立条目；没有标记集合页时，已启用的订阅文件为空。请从 `@local/typsite:0.1.0` 导入 `rss`，或合并新版 `root/lib/lib.typ` 的导入列表。`#rss(false)` 取消集合标记，不是条目排除开关。站点地图仍独立包含所有编译页面。

RSS 日期改为符合规范的 RFC 2822，不再使用 ISO 时间戳。纯日期及 `YYYY-MM-DD HH:MM[:SS]` 元数据按 UTC 处理，RFC 3339/RFC 2822 的时区偏移会转换为 UTC。无效日期会被省略；标题和摘要从 Typst 元数据中提取文本，不会直接显示 HTML 标签。非空但无效的站点 URL 现在会导致配置校验失败。

= 预览与失败恢复

仍通过非零的 `--port` 启动预览。所有已连接客户端会观察到同一构建版本；真正没有变化的轮询不会刷新浏览器。导出、解析或渲染失败时保留上次成功发布的页面，修复成功后触发刷新。按 Ctrl-C 会等待正在执行的构建结束，再退出。

即使后续发布失败，已经成功导出的结果也能复用。修复报错后，重新运行相同命令即可。发布只保证*单个文件*的原子替换，不是整个站点的事务：发布中途失败可能留下完整的新旧文件混合状态，普通重试会修复。如果部署要求站点整体切换，请另设部署步骤。
