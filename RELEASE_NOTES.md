<!--
  这份文件是 GitHub Release 正文的发版说明部分，由 .github/workflows/release.yml 读取。
  文末的「文档」章节是 workflow 自动拼上去的（链接要指向具体 tag，不能写死在这里），
  所以这里只写发版说明本身。

  注意正文格式：
     - **不要以 `# transgo x.y.z` 开头**。Release 页面的标题已经是版本号了，再写一遍是重复。
     - **不要写概括性的开场白**。读者点进 Release 就是来看改动细节的，直接从 `## 新增` 开始。

  发版流程：
    1. 重写本文件，写清这次的新增功能和改动
    2. git commit
    3. git tag v<版本号>        （要以 v 开头，例如 v0.1.0）
    4. git push --follow-tags

  workflow 会编译 x86_64 musl 静态二进制、打包、算校验和，然后建 Release 并把产物挂上去。
  同一份 workflow 能一直用下去，发新版不需要改它。
-->

## 新增

- shell 命令补全（可选）：`transgo completions zsh > ~/.zfunc/_transgo` 生成脚本，
  子命令和全部选项可 Tab 补全，不再需要背参数。bash / fish / elvish / powershell 同理。
  是否启用、怎么挂载见 README「命令补全」一节——transgo 不改动任何配置文件，用不用自行决定

## 文档

- 用法里说明引号规则：参数含空格必须加引号（翻译指令尤其要注意，否则指令的后半截会被
  当成待译文本），不含空格可省略；待译文本多词可以不引号，会拼回整句
