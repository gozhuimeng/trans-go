# 实现说明

面向打算改代码或加新引擎的人，用户请看 [README](../README.md)。

---

## 目录结构

```
crates/
├── transgo-core/           引擎抽象层，CLI 与 GUI 共用
│   └── src/
│       ├── lib.rs          HTTP client、超时等公共设施
│       ├── lang.rs         统一语种枚举 + 各家代码解析
│       ├── detect.rs       「是不是中文」判定 + 目标语决策
│       ├── types.rs        Request / Translation
│       ├── config.rs       config.toml 读写、环境变量覆盖、点分路径访问
│       ├── error.rs        统一错误类型 + 退出码映射
│       └── engine/
│           ├── mod.rs      Engine trait、注册表、默认引擎选择
│           ├── util.rs     报文抓取、错误解析、HTML 反转义等
│           ├── mymemory.rs     免注册
│           ├── deepl.rs        DeepL（免费 50 万/月）
│           ├── tencent.rs      腾讯云 TMT（TC3-HMAC-SHA256）
│           ├── volcano.rs      火山引擎（Volcengine V4 签名）
│           ├── aliyun.rs       阿里云（POP-RPC HMAC-SHA1）
│           ├── baidu.rs        百度通用翻译（MD5 签名）
│           ├── baidu_llm.rs    百度大模型文本翻译（Bearer API Key 或 MD5 签名）
│           ├── azure.rs        Azure AI Translator
│           ├── google.rs       Google Cloud Translation v2
│           ├── youdao.rs       有道智云（SHA-256 hex 签名）
│           └── openai.rs       任意 OpenAI 兼容接口
├── transgo-cli/            transgo 可执行文件
│   └── src/main.rs         参数解析、子命令、输出格式化
└── transgo-gui/            图形界面（eframe/egui），封装核心库
    └── src/lib.rs          翻译窗口、后台翻译线程、系统字体挂载
```

约 3500 行，含 20 个单元测试。

---

## 分层

```
                ┌─────────────┐   ┌─────────────┐
                │ transgo-cli │   │ transgo-gui │  (M2)
                └──────┬──────┘   └──────┬──────┘
                       │                 │
                       └────────┬────────┘
                                ▼
                        ┌──────────────┐
                        │ transgo-core │
                        └──────────────┘
```

`transgo-core` 拿到 `Request { text, from, to }` 就能返回 `Translation`，
完全不知道上层是 CLI 还是 GUI、也不关心语种方向是谁决定的。GUI 直接复用，不用改 core。

---

## Engine trait

```rust
#[async_trait]
pub trait Engine: Send + Sync {
    fn id(&self) -> &'static str;              // 稳定 id，用于 -e 和配置
    fn name(&self) -> &'static str;            // 展示名
    fn quota(&self) -> &'static str;           // 免费额度说明
    fn signup(&self) -> &'static str;          // 申请地址
    fn configured(&self) -> bool;              // Key 是否就位
    fn languages(&self) -> Option<&'static [Lang]>;  // None = 不限制

    fn supports(&self, req: &Request) -> bool { ... }  // 默认实现
    async fn translate(&self, req: &Request) -> Result<Translation>;
}
```

`languages()` 返回 `None` 表示不做白名单限制（LLM、MyMemory、阿里云、火山、有道）。
返回 `Some(&[...])` 的是语种范围确实有约束的：DeepL 23 个、Azure 29 个、腾讯 18 个、百度 22 个。

`pick_default()` 的选择顺序：

1. 配置里 `default.engine` 指定了就用它
2. 否则按 `engine::ORDER` 取第一个 `configured() && supports(req)` 的
3. 都不行就退回第一个 `supports(req)` 的（此时会报「未配置」让用户去填 Key）

`ORDER` 的排序依据是「国内可直连 + 免费额度 + 翻译质量」：

```rust
pub const ORDER: &[&str] = &[
    "deepl", "tencent", "volcano", "aliyun", "baidu", "baidu-llm", "azure", "google", "youdao",
    "llm", "mymemory",
];
```

`mymemory` 排在末尾，它免注册，保证没配任何 Key 时也能用。

---

## 语种统一层

各家语种代码差异极大，同一概念有 9 种写法：

| | 简中 | 繁中 | 日语 | 韩语 |
|---|---|---|---|---|
| **transgo** | `zh` | `zh-TW` | `ja` | `ko` |
| DeepL | `ZH` | `ZH-HANT` | `JA` | `KO` |
| 百度 | `zh` | **`cht`** | **`jp`** | **`kor`** |
| 有道 | **`zh-CHS`** | **`zh-CHT`** | `ja` | `ko` |
| 火山 | `zh` | **`zh-Hant`** | `ja` | `ko` |
| Azure | **`zh-Hans`** | `zh-Hant` | `ja` | `ko` |
| 腾讯 | `zh` | `zh-TW` | `ja` | `ko` |
| Google | `zh-CN` | `zh-TW` | `ja` | `ko` |
| 阿里云 | `zh` | `zh-tw` | `ja` | `ko` |

处理办法：

- `lang.rs` 定义一份通用代码（ISO 639-1 风格）
- 每个引擎自己写 `fn code(l: Lang) -> &'static str` 做映射，不搞中心化映射表，
  这样每个文件自包含，改一家不会影响另一家
- `Lang::from_code()` 负责输入侧的宽松解析，大小写、`_`/`-`、各家别名
  （`zh-CN` / `zh-Hans` / `chs` / `cht` / `jp` / `kr` / `iw`…）都能识别

---

## 目标语决策

规则只有一条：是中文就译成英文，其余译成中文。

判定在 `detect::is_chinese()`。设计要点：

- 看整段的字符构成比例（汉字占字母类字符的 1/3 以上），不看首字。否则「iPhone手机很好用」
  这类以英文开头的中文会被误判
- 数字、标点、空白不计入统计。它们在任何语言里长得都一样，掺进来只会稀释比例、
  把判定往阈值上拉，让结果不稳定
- 假名/谚文出现即判非中文。它们是日韩文专属字符，这样日译中、韩译中才会走「译成中文」分支

`detect::detect()` 是另一回事：它只用于**展示**（Azure 不回传检测到的源语种时给 `-v` / `--json` 的 `from` 字段补齐），
不影响译文方向。

### 已知局限

中文句子里夹一整个片假名词（「我很喜欢カタカナ」）会被判成日文。
这两种情况在字符统计上完全一样（汉字与假名各半），区分不了。
理论上可以用「假名游程是否与汉字交错」（日文送り仮名）来分辨，但不值得为这个边缘场景增加复杂度。
优先照顾日文是因为「日译中」是常态需求。

---

## 加一个新引擎

1. 在 `engine/` 下新建 `foo.rs`，实现 `Engine` trait
2. 如果语种代码跟通用代码不同，写 `fn code(l: Lang) -> &'static str`
3. `engine/mod.rs` 里 `mod foo;`，在 `build_all()` 的 `match` 里加一支
4. 把 id 加进 `ORDER`（决定它在默认选择里的优先位置）
5. `config.rs` 加对应的配置结构体，`apply_env()` 里加环境变量覆盖
6. `main.rs` 的 `TEMPLATE` 里加带注释的配置段
7. 报错处理参考 `engine/util.rs` 的 `api_err()` / `message_of()`，
   把各家的错误码翻译成中文提示（用户会为此感谢你）

签名类引擎（腾讯 TC3、阿里 POP-RPC、火山 V4、百度 MD5、有道 SHA-256）的实现都在各自文件里，
没有抽公共签名库，它们的算法差别太大，抽出来只会更难读。

---

## 踩过的坑

这些是实测出来的，改代码时别退回去。

### MyMemory 的主译文字段

必须取 `responseData.translatedText`。`matches[]` 是翻译记忆库命中列表，
同一段原文可能同时命中 `你好` 和 `您好`，而且排序在不同请求间会漂移。

### MyMemory 的源语种要用 `Autodetect`

未知源语时不能用本地探测结果冒充具体源语。实测 `zh-CN -> en-GB`：

| 输入 | 给具体源语 | `Autodetect` |
|---|---|---|
| `今天天气不错` | `today's Minnesota ideas`（错） | `It's a nice day today`（对） |
| `知识就是力量` | `Knowledge is power`（对） | `Knowledge is power`（对） |

给具体源语会触发翻译记忆库查询，命中质量是抽奖；`Autodetect` 走机翻反而稳定。

反过来也有反例（`hello world` 在 `Autodetect` 下返回 `hello world`，在 `en` 下返回 `你好世界`），
说明没有哪条路由稳定更好。这是数据质量问题，路由层面解决不了。

### 本地探测结果不能声称成源语种

实测把法语文本标成 `en` 发给 MyMemory，它会原样返回不翻译。
拉丁字母横跨英/法/德/西几十种语言，字符类别区分不了，猜错比不猜糟糕得多。

### SIGPIPE

Rust 的 `std` 启动时把 SIGPIPE 设为忽略，导致 `transgo lang | head` 直接 panic
（`failed printing to stdout: Broken pipe`）。

CLI 工具必须在 `main` 开头恢复 `SIG_DFL`。Unix 管道的语义就是「读端关了就安静退出」。

### 火山引擎 V4 签名的 `StringToSign`

官方文档三处写明第二行是完整的 `YYYYMMDDTHHMMSSZ`，但 `volc-openapi-demos` 的示例代码里
变量名叫 `date`，容易读成短日期 `YYYYMMDD`。

因为无法离线实测，实现里首次遇到签名错误会自动换一种形式重试一次，并用 `AtomicU8` 记住结论，
后续调用直接走正确形式。**如果你有火山的 Key 验证过，可以把这个探测逻辑删掉。**

### 百度大模型文本翻译的 `salt`

官方文档说 `salt` 是「可为字母或数字的字符串」，那是通用翻译 API 的规矩。
大模型文本翻译的 JSON 体里 `salt` 实际必须是 int64 数字：传字符串报 `53001`
（parse json body error: readUint64），超过 int64 上限也报 `53001`（ReadInt64: overflow）。
签名拼接时用它的十进制形式，与通用翻译一致。`baidu_llm.rs` 里取 UUID 低 63 位即为此故。

### 各家的错误码类型不稳定

- 百度 `error_code` 在示例里是字符串、在参数表里是整数
- 阿里云 `Code` 在成功时是整数 `200`、在网关报错时是字符串
- 有道 `errorCode` 是字符串（`"0"` 才是成功）

解析时都要做类型宽容处理。

### 中文终端对齐

中文在终端占两列，按 `chars().count()` 对齐表格会错位。
`main.rs` 里有 `dwidth()` / `pad()` 按显示宽度补齐。

### clap 的位置参数与子命令歧义

clap 无法在一个命令里同时容纳「贪婪的位置参数」（`[TEXT]...`）和「子命令」而不产生歧义。
`main.rs::normalize_argv()` 在解析前做一次纯字符串预扫描，把默认子命令 `translate` 补上，
让 `transgo "hello"` 与 `transgo translate "hello"` 等价。判定是确定性的，不依赖 clap 的启发式。

文本参数不设 `trailing_var_arg` / `allow_hyphen_values`：设了会让第一个位置参数之后的
`-v` 这类选项被吞成待翻译文本（`transgo "hello" -v` 曾译出 `你好-v`）。代价是以 `-` 开头的
文本要放到 `--` 之后（`transgo -- -v`），裸 `-` 仍是 stdin 标记。多词文本不用引号，
分开传会按空格拼回。

---

## 测试

```console
$ cargo test
```

20 个单元测试，重点覆盖容易出错的部分：

- `detect.rs`：中文判定的边界（英文开头的中文、中文夹英文词、数字标点稀释、日韩文）
- `config.rs`：点分路径读写、标量解析
- `youdao.rs`：`truncate()` 的 UTF-16 码元语义（要跟有道官方 JS SDK 对齐，不是字符数）
- `tencent.rs`：SHA-256 输出稳定性
- `openai.rs`：剥掉模型偶尔加的外层引号（但保留文本内部的引号）
- `util.rs`：HTML 实体反转义顺序（`&amp;` 必须放最后）

签名类引擎的请求构造没法离线单测（需要真实 Key），用 `transgo engines --test` 做集成验证。

---

## 发布前自查

```console
$ cargo test
$ cargo clippy --all-targets
$ cargo build --release
$ transgo engines            # 确认没有 warning 引起的行为变化
$ transgo lang | head -3     # 确认 SIGPIPE 修复还在
```
