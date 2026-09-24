# transgo

轻量快速的翻译工具，CLI 优先，Wayland / Hyprland 友好。

只接提供商官方翻译接口，不使用任何逆向网页版的反代。

```console
$ transgo "知识就是力量"
Knowledge is power

$ echo "Actions speak louder than words" | transgo
事实胜于雄辩

$ transgo -t ja "你好"
こんにちは
```

---

## 特性

- stdout 只有译文本身，元信息一律走 stderr，可以直接进管道，也能直接喂给 `wl-copy`
- 11 个官方翻译接口，免费额度从 5 万到 500 万字符/月不等，挑顺手的填 Key 就行
- 语种方向自动决策，出问题时可以手动覆盖
- 单二进制，运行期零外部依赖，不调 `curl`、不调 `wl-paste`，拷过去就能跑
- 不监听全局按键、不跑守护进程，快捷键由桌面环境直接调起
- 图形界面随手可开：`transgo gui` 空白窗口，`transgo gui --clip` 预填剪贴板并直接翻一次。
  回车翻译、Shift + 回车换行、译文一键复制

为什么强调 Wayland 友好？Wayland 出于安全设计不允许应用自行监听全局按键。pot 一类工具在
Hyprland 下「注册不了快捷键、只能开端口绕过」，根因就在这里，跟实现质量没关系。transgo 不跟
这个限制较劲：CLI 就是集成入口，由 `bind ... exec` 调起，100% 可靠。

---

## 安装

```console
$ cargo build --release          # 产物在 target/release/transgo
$ cargo install --path crates/transgo-cli
```

需要 Rust 1.75+，不用装系统开发库。

装完可以先跑一次 `transgo engines`，它会告诉你哪些引擎还没配 Key。

---

## 快速开始

不填任何 Key 就能用（走免注册的 MyMemory）：

```console
$ transgo "今天天气不错"
The weather is nice today
```

想用质量更好、额度更大的接口，配上 Key 即可。没配的引擎会被自动跳过，不用手动开关：

```console
$ transgo config set deepl.api_key 你的Key
$ transgo "今天天气不错"
It's a nice day today
```

各家的申请步骤、免费额度、超额计费方式和开通门槛见 [docs/engines.md](docs/engines.md)。

---

## 用法

`transgo` 后面直接跟文本即可，`translate` 子命令可省略。

```console
$ transgo "hello world"                  # 方向自动决策
$ echo "good morning" | transgo          # 从 stdin 读，管道友好
$ transgo -t ja "你好"                    # 指定目标语
$ transgo -f en -t zh "hello"            # 同时指定源语
$ transgo -e deepl "hello"               # 指定引擎
$ transgo --json "hello"                 # 完整结果，便于脚本处理
$ transgo -v "hello"                     # stderr 里附引擎、语种、耗时
$ transgo -a "hello"                     # 附带备选译文
```

完整选项：

```console
$ transgo translate --help
翻译文本（默认命令，可省略）

Usage: transgo translate [OPTIONS] [TEXT]...

Arguments:
  [TEXT]...  要翻译的文本，不传或传 - 则从 stdin 读取；以 - 开头的文本放到 -- 之后

Options:
  -t, --to <LANG>           目标语种，如 zh / en / ja；auto = 中文译英文，其余译中文
  -f, --from <LANG>         源语种，不填就自动检测
  -e, --engine <ID>         翻译引擎，不填则取第一个已配置的
  -i, --instruction <TEXT>  翻译指令，控制文风（仅 baidu-llm 生效），如「采用意译」
  -j, --json                用 JSON 输出完整结果
  -a, --alternatives        备选译文打到 stderr，stdout 保持干净好接管道
  -v, --verbose             在 stderr 附上引擎、语种和耗时
  -h, --help                Print help
  -V, --version             Print version
```

### 其他子命令

| 命令 | 作用 |
|---|---|
| `transgo engines` | 列出引擎、免费额度、配置状态 |
| `transgo engines --test` | 对每个已配置引擎实发一次请求，验证 Key 可用 |
| `transgo engines -v` | 附带申请地址 |
| `transgo lang` | 列出支持的语种 |
| `transgo lang -e baidu` | 只看某个引擎支持的语种 |
| `transgo gui` | 打开图形界面翻译窗口 |
| `transgo gui --clip` | 窗口预填剪贴板内容并直接翻译 |
| `transgo config path` | 配置文件路径 |
| `transgo config list` | 列出全部配置项 |
| `transgo config get <KEY>` | 读取单项 |
| `transgo config set <KEY> <VALUE>` | 写入单项（值传空串表示清除） |
| `transgo config unset <KEY>` | 清除单项 |
| `transgo config init` | 生成带注释的模板配置 |
| `transgo config edit` | 用 `$EDITOR` 打开配置 |

退出码：`0` 成功 · `1` 网络 / IO · `2` 配置或用法 · `3` 远端接口报错。
脚本里可以据此区分是「没配 Key」还是「上游挂了」。

---

## 语种规则

默认 `--to auto`，规则只有一条：

| 原文 | 目标语 |
|---|---|
| 是中文 | 英语 |
| 不是中文 | 简体中文 |

判定看整段文本的字符构成比例（汉字占字母类字符的 1/3 以上），不看首字，所以
「iPhone手机很好用」这类以英文单词开头的中文不会被误判。数字、标点、空白不计入统计，
它们在任何语言里长得都一样，掺进来只会让判定不稳定。假名/谚文是日韩文专属字符，
出现即判非中文，日译中、韩译中因此都走「译成中文」这条分支。

已知局限：中文句子里夹一整个片假名词（如「我很喜欢カタカナ」）会被判成日文。
这两种情况在字符统计上完全一样，区分不了；优先照顾日文是因为「日译中」更常见。

### 出问题时手动覆盖

```console
$ transgo -t ja "你好"                    # 改目标语（偶尔会用到）
$ transgo -f en -t zh "hello"             # 同时指定源语
```

任意语种代码都能识别，大小写、`_`/`-`、各家别名均可（`zh-CN` / `zh-Hans` / `chs` 等价），
完整列表见 `transgo lang`。

---

## 翻译引擎

全部是提供商官方接口，免费额度核实于 2026-09。

| 引擎 | 免费额度 | 需要什么 |
|---|---|---|
| `deepl` | 50 万字符/月，永久 | 免费注册拿 Key |
| `tencent` | 500 万字符/月 | SecretId + SecretKey |
| `volcano` | 200 万字符/月 | AccessKey |
| `azure` | 200 万字符/月 | 订阅 + Key（需绑卡） |
| `aliyun` | 100 万字符/月 | AccessKey |
| `baidu` | 标准版 5 万/月<br>实名认证后高级版 100 万/月 | APP ID + 密钥 |
| `baidu-llm` | 实名认证后一次性 100 万字符测试额度 | APP ID + 密钥 |
| `google` | 50 万字符/月，永久 | GCP 项目 + Key（需绑卡开 billing） |
| `llm` | 取决于服务商 | 任意 OpenAI 兼容接口 |
| `mymemory` | 5 千字符/天（留邮箱 5 万） | 免注册 |
| `youdao` | 无月度额度，仅一次性体验金 | AppKey + AppSecret |

各家的额度和超额行为见 [docs/engines.md](docs/engines.md)。
`mymemory` 免注册，排在优先级末尾。

> `mymemory` 质量有限。它是众包翻译记忆库，译文取自其他用户提交的条目，抽到哪条看运气。
> 典型症状是把术语表条目当译文原样返回（`hello world` → `hello world`），这是数据问题，换路由解决不了。
> 它的定位是没配任何 Key 时先能用，配好 `deepl` / `tencent` 后会自动被跳过。

未填 Key 的引擎会被自动跳过，无需手动开关。默认引擎按上表顺序取第一个已配置的，
可用 `transgo config set default.engine <id>` 固定。

> 没接的两个：`Amazon Translate` 的免费层是 12 个月限时，且 2025-07-15 之后注册的账号已取消；
> `彩云小译` 是一次性体验额度而非月度。都不值得接。

每个引擎怎么申请 Key、有什么坑，见 [docs/engines.md](docs/engines.md)。

---

## 配置

路径 `~/.config/transgo/config.toml`（可用 `TRANSGO_CONFIG` 覆盖）。

```console
$ transgo config init             # 生成带注释的模板
$ transgo config edit             # 用 $EDITOR 打开
$ transgo config set deepl.api_key 你的Key
```

```toml
[default]
engine = ""            # 留空则按优先级取第一个已配置的
to = "auto"            # auto = 中文译英文，其余译中文
from = "auto"
timeout_secs = 15

[deepl]
api_key = ""           # 免费 Key 以 `:fx` 结尾，会自动走 api-free.deepl.com

[llm]
api_key = ""
base_url = "https://api.openai.com/v1"
model = "gpt-4o-mini"
# system_prompt = ""   # 可覆盖默认提示词，用来固定术语表或语气
```

环境变量优先于配置文件，方便脚本临时覆盖：
`TRANSGO_DEEPL_API_KEY`、`TRANSGO_TENCENT_SECRET_ID`、`TRANSGO_LLM_MODEL`…
完整列表见 `transgo config init` 生成的模板注释。

配置文件写入后权限自动收紧到 `600`（里面有 API Key）。
写回时只持久化文件里已有的值，不会把当前 shell 的环境变量一起写进去。

---

## 桌面集成

```bash
# ~/.config/hypr/hyprland.conf

# 翻译剪贴板内容，结果写回剪贴板
bind = SUPER, T, exec, wl-paste | transgo | wl-copy

# 翻译剪贴板内容并弹出通知
bind = SUPER SHIFT, T, exec, notify-send "译文" "$(wl-paste | transgo)"
```

stdout 只有译文本身，所以也能直接进 `rofi`、`dmenu`、`fzf` 的管道。

更多脚本、窗口规则，以及 Wayland 全局快捷键的限制说明，见 [docs/hyprland.md](docs/hyprland.md)。

---

## 遇到问题

```console
$ transgo engines --test          # 先跑这个，逐个验证 Key 是否可用
```

报错信息对照、配置不生效排查，见 [docs/troubleshooting.md](docs/troubleshooting.md)。

---

## 路线图

- [x] **M1** CLI 主体 + 11 个官方翻译引擎 + 配置管理
- [x] **M2** GUI（`transgo gui` / `transgo gui --clip`）
      独立输入翻译窗口，封装 CLI 的核心库；不监听剪贴板、不监听全局按键
- [ ] 划词翻译（模拟 Ctrl+C + 读剪贴板，需装 `wtype`）
- [ ] 截图 OCR 翻译（`grim` + `slurp` + `tesseract`）
- [ ] 生词本 / 历史记录

---

## 文档索引

| 文档 | 读者 | 内容 |
|---|---|---|
| [docs/engines.md](docs/engines.md) | 使用者 | 11 个引擎怎么申请 Key、怎么配、有什么坑 |
| [docs/hyprland.md](docs/hyprland.md) | 使用者 | 桌面集成、快捷键绑定、实用脚本 |
| [docs/troubleshooting.md](docs/troubleshooting.md) | 使用者 | 报错对照、自查步骤 |
| [docs/architecture.md](docs/architecture.md) | 开发者 | 目录结构、引擎抽象、踩过的坑、怎么加新引擎 |

## License

MIT

本项目由 mimo v2.6 完成开发
