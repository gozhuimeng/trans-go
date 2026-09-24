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

- 翻译指令（`baidu-llm`）：`transgo -i "采用意译" "原文"` 控制文风，或配置 `baidu.reference`
  设默认指令；GUI 里是「翻译指令」折叠区。上限 500 字符，官方声明该功能免费
- 术语库干预（`baidu-llm`）：`baidu.need_intervene = true` 后，译文遵循控制台
  「我的术语库」里的术语对照表，功能免费

## 修复

- `llm` 引擎提示词里的语种改为英文全名（Simplified Chinese 这类）。小模型对「Zh」
  这类短代码服从不稳，目标语会漂到随机语种；换全名后，小型翻译模型也能稳定按指令出译文
- `llm` 引擎的输出清理会剥掉小模型偶尔回显的分隔线
- `llm` 引擎对接局域网自建服务（10.x / 172.16-31.x / 192.168.x）不再强制 API Key，
  公网服务商照旧必须填写
- 修正 workspace 的 repository 元数据指向实际仓库
