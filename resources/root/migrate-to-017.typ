#import "/lib/lib.typ": *

#show : schema.with("page")

#let version = html.text(fill: rgb("#22D3EE"))[0.1.7]

#title[迁移到 Typsite #version]
#page-title[迁移到 Typsite 0.1.7]
#date[2026-03-02 18:25]
#author[Glomzzz]
#parent("index.typ")

#let breaking-changes = html.text(fill: red.darken(10%))[breaking-changes]

在 #version 版本中，Typsite 进行了一系列关于SVG兼容的更新，其中不可避免地包括一些#breaking-changes ，遂纂写此文档以帮助用户更好地迁移到新版本。

= 0.1.6 迁移

如果你从 0.1.6 之前的版本迁移，请确保也完成了 0.1.6 的迁移步骤：#cite-title("migrate-to-016.typ")


= .typsite 配置迁移

在 #version 版本中，.typsite 目录新增了以下组件和重写器：

- `.typsite/components/anchor_def_svg.html`：SVG 锚点定义组件
- `.typsite/components/anchor_goto_svg.html`：SVG 锚点跳转组件
- `.typsite/rewrite/footnote-ref-svg.html`：脚注引用的 SVG 重写器

如果你是从旧版本升级的，请确保将这些新文件复制到你的站点 .typsite 目录中。

= /root/lib 库迁移

== inline 函数签名变更

`inline` 函数的 `fit-font` 参数语义发生了变更：

- #version 之前：`fit-font` 为 `bool` 类型
- #version 之后：`fit-font` 为 `1em` 类型（可以是 `true`、`false` 或 `1em`、`2em` 等 em 值）

迁移方式：
- `inline(content, fit-font: true)` |-> `inline(content, fit-font: 1em)`
- `inline(content, fit-font: false)` |-> `inline(content)` 或 `inline(content, fit-font: false)`

== 库导入变更

如果你在站点库中自定义了 `inline` 函数，需要更新调用方式：

```typ
// 旧版本
#import "@local/typsite:0.1.0": inline

// 新版本
#let footnotes = state("footnotes", ())
#let inline(..args) = context {
  import "@local/typsite:0.1.0": inline
  inline(..args, footnotes: footnotes.get())
}
```

== rule 函数签名变更

`rule.typ` 中的以下函数需要接收 `footnotes` 参数：

- `rule-ref(footnotes)` 替代旧的 `rule-ref`
- `rule-footnote(footnotes)` 替代旧的 `rule-footnote`

在 `schema` 函数中，需要先获取 footnotes 再传递给 rule：

```typ
let _footnotes = query(footnote).map(it => it.at("label", default: none)).filter(it => it != none)
show: rule-footnote(_footnotes)
show: rule-ref(_footnotes)
```

= /packages 包迁移

== inline 函数新增 footnotes 参数

`@local/typsite:0.1.0` 中的 `inline` 函数现在需要接收 `footnotes` 参数：

```typ
// 旧版本
inline(content, fit-font: true, scale: 100%)

// 新版本
inline(content, fit-font: 1em, scale: 100%, footnotes: ())
```

== auto-sized-svg 新增 fit-font 参数

`auto-sized-svg` 函数新增了 `fit-font` 参数，支持更多字体大小控制方式：

- `true`：使用 CSS font-size 继承
- `false`：使用固定 font-size: 1rem
- `1em`：使用相对 em 值

= Zed 编辑器兼容

== Zed LSP 配置

如果你使用 #link("https://zed.dev")[Zed] 作为编辑器，可以使用以下配置来获得更好的 Typst 开发体验：

在 `.zed/settings.json` 中添加：

```json
{
  "lsp": {
    "tinymist": {
      "initialization_options": {
        "exportPdf": "never",
        "rootPath": "你的站点根目录/root",
      },
    },
  },
}
```

== VSCode 设置迁移

如果你之前使用 VSCode 并配置了 tinymist，需要更新 `rootPath` 为绝对路径：

在 `.vscode/settings.json` 中：

```json
{
  "tinymist.rootPath": "/absolute/path/to/your/site/root"
}
```


= 其他更新

- 新增 SVG 锚点支持，可以在 Typst 内容中创建可跳转的锚点
- 脚注引用现在支持 SVG 内联渲染
- 改进了 `inline` 函数的字体大小自适应机制
- 支持更灵活的 `fit-font` 参数，可以指定具体的 em 值
