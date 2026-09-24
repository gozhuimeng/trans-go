<!--
  这份文件是 GitHub Release 正文的发版说明部分，由 .github/workflows/release.yml 读取。
  文末的「文档」章节是 workflow 自动拼上去的（链接要指向具体 tag，不能写死在这里，
  含中英文两个链接块），所以这里只写发版说明本身。

  注意正文格式：
     - **不要以 `# transgo x.y.z` 开头**。Release 页面的标题已经是版本号了，再写一遍是重复。
     - **不要写概括性的开场白**。读者点进 Release 就是来看改动细节的，直接从第一个章节
       标题开始；有行为变化时 `## 行为变化` 置顶，其余从 `## 新增` 开始。
     - **双语**：中文在前，用 `---` 分隔后重复一遍英文版（英文部分的文件链接指向英文文档）。

  发版流程：
    1. 重写本文件，写清这次的新增功能和改动（中英双语）
    2. git commit
    3. git tag v<版本号>        （要以 v 开头，例如 v0.1.0）
    4. git push --follow-tags

  workflow 会编译 x86_64 musl 静态二进制、打包、算校验和，然后建 Release 并把产物挂上去。
  同一份 workflow 能一直用下去，发新版不需要改它。
-->

## 新增

- 界面语言可配置：`ui.lang`（`zh` / `en`，默认 `zh`；环境变量 `TRANSGO_UI_LANG`）。
  一个字段同时切换 CLI 帮助、报错文案、配置模板、GUI 文字，以及引擎列表里的名称与额度。
  引擎的错误提示括注与 CLI 表格标签暂为中文

## 修复

- GUI 启动失败时给出明确报错，此前只透传底层错误信息

---

## Added

- Configurable interface language: `ui.lang` (`zh` / `en`, default `zh`; env
  `TRANSGO_UI_LANG`). One field switches CLI help, error messages, the config template, GUI
  text, and the engine names and quotas in listings. Engine error hints and CLI table labels
  remain Chinese for now

## Fixes

- GUI startup failures now report a clear message instead of raw underlying errors
