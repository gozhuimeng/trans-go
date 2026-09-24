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

## 行为变化

- `auto` 方向的第三方语言（日韩俄阿等非拉丁文字）现在默认翻向**英文**，此前是翻向中文。
  想要旧行为，一行配置即可：`default.lang = "zh"`

## 新增

- 第三方语言方向可配置：`default.lang`（`zh` / `en`，默认 `en`；环境变量
  `TRANSGO_DEFAULT_LANG`）。中英之间永远互转，不受它影响
- 英文文档：`README_EN.md` 与 `docs_en/`（引擎指南、故障排查、桌面集成、实现说明）。
  Release 正文从此中英双语，页尾文档链接也是双语

## 修复与改进

- 文档中期体检：修正额度范围、退出码语义、窗口规则匹配方式（class 改 title）等 26 处
  事实问题，补上「Google 超出会从绑定的卡实际扣费」的警示
- 各引擎标注实测状态：4 个用真实请求验证过，7 个未实测
- 欢迎提 Issues 与 PR

---

## Behavior change

- Third-party languages (Japanese, Korean, Russian, Arabic and other non-Latin scripts) now
  translate to **English** by default under `auto`; previously to Chinese. The old behavior is
  one config line away: `default.lang = "zh"`

## Added

- Configurable third-language direction: `default.lang` (`zh` / `en`, default `en`; env
  `TRANSGO_DEFAULT_LANG`). Chinese and English always swap and are unaffected
- English documentation: `README_EN.md` and `docs_en/` (engine guide, troubleshooting,
  desktop integration, implementation notes). Release notes are bilingual from now on, and the
  docs links at the foot of the page come in both languages

## Fixes and improvements

- Mid-term documentation audit: 26 factual fixes (quota ranges, exit-code semantics, window
  rule matching — class to title — and more), plus a warning that Google charges the bound
  card beyond the free quota
- Per-engine test status documented: 4 verified with real requests, 7 untested
- Issues and PRs welcome
