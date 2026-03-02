#import "/lib/lib.typ": *

#show : schema.with("page")

#let version = html.text(fill: rgb("#22D3EE"))[0.1.6]

#title[迁移到 Typsite #version]
#page-title[迁移到 Typsite 0.1.6]
#date[2025-06-21 02:47]
#author[Glomzzz]
#parent("index.typ")

#let breaking-changes = html.text(fill: red.darken(10%))[breaking-changes]

在 #version 版本中，Typsite 进行了一系列重大更新，其中不可避免地包括一些#breaking-changes ，遂纂写此文档以帮助用户更好地迁移到新版本。

= Typsite Packages

将 Typst 库中与 typsite 紧密相关的部分单独抽离成一个Typst package，你可以在 #link("https://github.com/Glomzzz/typsite/tree/main/packages/")[typsite/packages] 下找到与 typsite 的各个 packages。typsite 会将这些 packages 打包到二进制文件中，在每次 `typsite compile` 时会自动将这些 packages 安装到 `@local` 目录下

当然，为了开发者调试包与使用包的方便，typsite 也提供了本地包自动安装，你只需要在 `typsite compile` 时指明 `--packages` 参数即可
- 如果在 `watch-mode` 下，typsite 也会自动更新这些本地包并重新编译站点

所以，现在 typsite 的 Typst 库分为了两个部分：
- `@local/typsite:0.1.0`：包含了 typsite 的核心功能，提供各种基础函数，充当 typsite 与 站点库 之间的接口
    - 通过这层接口，未来将很难有更多的 #breaking-changes
- `root/lib/`: 每一个 typsite 站点的站点库，用户可以自定义其中的任何内容

= 迁移

如果您依赖了 #version 之前的站点库，迁移到 #version 版本时，您需要做如下替换：
- `import "lib.typ"` |-> `import "/lib/lib.typ"`
- `html-text` |-> `html.text`
- `text-align` |-> `html.align`
  - (注意不要把 `/lib/site.typ` 中的 `text-align` 替换掉了)

推荐通过绝对路径去`import` `lib.typ`，如果您使用的是 #link("https://github.com/Myriad-Dreamin/tinymist")[tinymist] 作为LSP, 您需要指定 Typst 的 `root-path` 来获取更好的体验：
- 在 VSCode 中，您可以在 站点目录的 `.vscode/settings.json` 中添加如下配置：
  ```json
  "tinymist.rootPath": "站点目录的绝对路径/root"
  ```

对于我在此没有预期到的 #breaking-changes ，您可以非常便捷地通过 `typsite compile --port 8000` 开启 `watch-mode`，typsite会在您每次修改文件时给予您错误提示，您可以根据提示进行修复。

= 其他
- 支持了 Typst math -> mathml 的转换
  - 这使得 typsite 可以更好地支持数学公式的展示，并且 mathml 转换会就地检测 Typst 的 math-font 并做兼容
- `inline` 函数新增 `fit-font` 参数， 可以自适应 Typst 的 `context text.size`
- 对于包含 Backlinks 与 References 的页面，`<head>` 内的部分进行了去重优化，现在非常整洁
- 在默认的站点库中，对 `show-rules` 进行了模块化，你可以非常方便的在 `/lib/lib.typ` 中添加/删除/修改 `show-rule`
- 在默认站点配置中，优化了连接的 `title`, 你可以预览到 `连接的标题 [链接地址]` 了
