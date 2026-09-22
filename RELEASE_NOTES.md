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

- 翻译走命令行。`transgo "知识就是力量"` 传文本就译，`echo "good morning" | transgo` 走管道。
  stdout 只有译文本身，元信息一律走 stderr，接 `wl-copy` 或自己的脚本都不脏
- 10 个翻译引擎，全是提供商官方接口，不用逆向网页版的反代：
  DeepL、腾讯云、火山引擎、阿里云、百度、Azure、Google Cloud、有道、任意 OpenAI 兼容接口、MyMemory
- 免费额度从 5 万到 500 万字符/月不等。不填 Key 的引擎自动跳过，
  零配置也能用（MyMemory 免注册兜底）
- 语种方向自动决策：中文译英文，其余译中文。判定看整段字符构成比例，不看首字，
  所以「iPhone手机很好用」这种以英文开头的中文不会被误判。
  真判错了，`-t` / `-f` 手动指定
- 各家语种代码差异统一屏蔽了。百度的 `jp`、有道的 `zh-CHS`、火山的 `zh-Hant`、
  Azure 的 `zh-Hans` 都不用记，只管写 `ja` / `zh-TW`
- `engines` 列出引擎、免费额度和配置状态，加 `--test` 会实发请求验证 Key 能不能用
- 配置归 `config` 管，`init` 生成带注释的模板，写入后权限自动收紧到 600
- `lang` 列语种，`-e` 可以只看某个引擎支持的范围

## 使用

下载对应平台的压缩包，解压后把 `transgo` 放进 `PATH` 就行。
是 musl 静态编译的单文件，不依赖系统库，任何 x86_64 Linux 发行版都能跑。

```console
$ transgo config set deepl.api_key 你的Key
$ transgo "知识就是力量"
Knowledge is power
```

## 已知限制

MyMemory 是众包翻译记忆库，译文取自其他用户提交的条目，抽到哪条看运气，
偶尔会把术语表条目当译文原样返回。它是没配任何 Key 时的兜底，
配好 DeepL 或腾讯云之后会被自动跳过。

## 下一步

GUI 独立窗口、划词翻译、截图 OCR 翻译、生词本。
