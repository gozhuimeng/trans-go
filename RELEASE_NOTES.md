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

- 新引擎 `baidu-llm`，接百度大模型文本翻译 API。与通用翻译（`baidu`）共用一套 APP ID + 密钥，
  两套免费额度分开计算：通用翻译按月发放（个人认证 100 万字符/月），
  大模型翻译是实名认证后自动发放的一次性 100 万字符测试额度，不按月重置。
  鉴权两种任选：API Key（Bearer），或与通用翻译相同的 MD5 签名
- 百度两个接口的差异、免费额度、计费时序（次日结算、余额不足即停、无自动充值）
  和官方错误码对照补进文档。引擎报错提示从 6 条扩到 15 条，全部对齐官方口径

## 修复

- `transgo config set` 传纯数字（比如百度 APP ID）会因被解析成数字而写入失败，
  现在会按原样存进字符串字段
- 百度捷克语代码 `cze` 改成官方对照表的 `cs`
- 腾讯云服务未开通的错误码改为官方的 `FailedOperation.UserNotRegistered`，
  并补上 `NoFreeAmount`（本月免费额度用完）、`ServiceIsolate`（欠费停服）、
  `ErrorUserArea`（账号区域与请求不符）的对照
