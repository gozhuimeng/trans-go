# 翻译引擎申请与配置指南

transgo 只接提供商官方接口。本文档告诉你每个引擎的 Key 去哪申请、怎么填、有什么坑。

全部免费额度核实于 2026-09。**申请页面会改版，若步骤对不上以官网为准。**

## 实测状态

`baidu`、`baidu-llm`、`llm`、`mymemory` 用真实请求实测过双向翻译；
`deepl`、`tencent`、`volcano`、`aliyun`、`azure`、`google`、`youdao` 未实测。

用的时候遇到问题，欢迎提 [Issues](https://github.com/gozhuimeng/trans-go/issues)；
定位到原因的，更欢迎直接提 [PR](https://github.com/gozhuimeng/trans-go/pulls)。

---

## 各家一览

下表只列事实：额度怎么给、超出后怎么计费、开通有什么门槛。选哪家自行判断。

| 引擎 | 免费额度 | 超出后 | 开通门槛 |
|---|---|---|---|
| `deepl` | 50 万字符/月 | 停服，不扣费 | 要 DeepL 支持的国家和地区发行的信用卡，国内的 Visa 双币卡外币卡不认 |
| `tencent` | 500 万字符/月 | 后付费默认关闭，用完停服 | 2026-10-01 起停发免费资源包，2027-10-01 全部接口不可用 |
| `volcano` | 每月前 200 万字符 | 49 元/百万字符，自动扣费，下月初结算 | 实名认证 |
| `aliyun` | 100 万字符/月 | 50 元/百万字符，自动转后付费且无法关闭 | 实名认证 |
| `baidu` | 标准版 5 万/月；个人认证后高级版 100 万/月 | 49 元/百万字符，次日结算，余额不足即停服 | 高级版要实名认证 |
| `baidu-llm` | 实名认证后一次性 100 万字符测试额度 | 49 元/百万字符，次日结算，余额不足即停服 | 要实名认证；大模型文本翻译服务单独开通 |
| `azure` | 200 万字符/月 | 见 Azure 文档 | 要 Azure 订阅 |
| `google` | 50 万字符/月 | 见 Google 文档 | 要绑 billing，且国内要代理 |
| `youdao` | 一次性 50 元体验金 | 余额制，扣完即停 | 无 |
| `llm` | 取决于服务商 | 取决于服务商 | 无 |
| `mymemory` | 5 千字符/天（留邮箱 5 万） | 无扣费概念 | 免注册 |

配好之后跑一次确认：

```console
$ transgo engines --test
```

每行会显示 `OK` / `失败 <原因>`，一眼看出哪把 Key 是好的。

---

## DeepL

**免费额度**：50 万字符/月，永久有效，用完直接停服，不会扣费。

**申请门槛（国内过不去）**：DeepL 要求用其支持的国家和地区发行的 VISA / MASTER
信用卡开通 API，**国内发行的信用卡一律不认**，双币卡、外币卡也不行。这不是国别禁令
（受限地区名单里没有中国大陆），卡在发卡行。有境外卡才能走下面的步骤。

**申请步骤**

1. 打开 <https://www.deepl.com/pro-api>，注册（免费账户即可）
2. 进入 API → API Keys，创建一个 Key
3. 免费 Key 以 `:fx` 结尾，这是 DeepL 官方对免费账户的标记

**配置**

```console
$ transgo config set deepl.api_key 你的Key
```

**坑**

免费 Key 走 `api-free.deepl.com`，Pro Key 走 `api.deepl.com`。transgo 按 Key 是否以 `:fx` 结尾
自动选择，一般不用管；自建网关或 Key 形状特殊时可以显式指定：

```console
$ transgo config set deepl.plan pro        # free 或 pro
$ transgo config set deepl.endpoint https://你的网关/v2/translate
```

报错记两个：`403` 是 Key 无效，或者免费 Key 被发到了 Pro 接口（检查 `deepl.plan`）；
`456` 是本月 50 万字符用完了，下月 1 日重置。

---

## 腾讯云机器翻译（2027-10-01 停服）

2026-09-03 公告《QPS 调整与免费资源包下线》：

| 时间 | QPS |
|---|---|
| 2026-09-30 及以前 | 无缩减 |
| 2026-10-01 至 2027-01-31 | 每月在上月基础上缩减 5% |
| 2027-02-01 至 2027-09-30 | 每月缩减 10% |
| 2027-10-01 起 | 缩减至零，**所有接口不可使用** |

免费资源包 2026-10-01 起停止发放；此前已领的在有效期内能用，过期不再续发。
由于资源包仅当月有效，免费额度实际到 2026-09-30 就断了。

下面是给已经在用、想撑到最后一刻的人留的。**免费额度**：文本翻译每月 500 万字符。
以资源包形式发放：当月首次开通当天就发，之后每月 1 日发，仅当月有效。
**后付费默认关闭**，资源包用完自动停服，不会转成扣费。

**申请步骤**

1. 打开 <https://console.cloud.tencent.com/tmt>，勾选服务协议后点「开通」，文本翻译等接口一键全开
2. 去 <https://console.cloud.tencent.com/cam/capi> 创建 API 密钥，拿到 SecretId 和 SecretKey

**配置**

```console
$ transgo config set tencent.secret_id 你的SecretId
$ transgo config set tencent.secret_key 你的SecretKey
# $ transgo config set tencent.region ap-guangzhou     # 默认就是这个，一般不用改
```

**坑**

必须先开通服务，只建密钥不够。SecretKey 是敏感信息，别提交进 git。
子账号要有 `QcloudTMTFullAccess` 之类的权限，否则报 `UnauthorizedOperation`。

报错对照：

| 错误码 | 含义 |
|---|---|
| `FailedOperation.UserNotRegistered` | 服务没开通。Key 是好的，去 <https://console.cloud.tencent.com/tmt> 点「开通」 |
| `FailedOperation.NoFreeAmount` | 本月 500 万字符用完了，下月 1 日重置 |
| `FailedOperation.ServiceIsolate` | 账号欠费停服 |
| `FailedOperation.ErrorUserArea` | 账号区域和请求的 `region` 对不上 |
| `SignatureDoesNotMatch` | SecretKey 抄错了 |
| `AuthFailure.SignatureExpire` | 系统时钟不准，`timedatectl` 校准一下 |

---

## 火山引擎机器翻译

**免费额度**：文本翻译每月前 200 万字符免费，超出按 49 元/百万字符。

注意控制台那张「刊例价」表只列超出后的单价，不写免费额度，容易误以为没有。阶梯是
`[0, 2] 百万字符 0 元，(2, +∞) 49 元/百万字符`，见官方《产品计费--机器翻译》。

**没有硬性刹车。** 火山开通即默认「日结按量后付费」，超出免费额度后自动继续扣费、
不会停服，账单要到下月初结算才看得见。这跟腾讯云的「后付费默认关闭、用完停服」相反。

费用中心可设置额度预警：<https://console.volcengine.com/cost/expense> 充 1 元，把「可用额度
预警」设成 1 元，扣款时会收到通知。

**申请步骤**

1. 打开 <https://console.volcengine.com/ai/region:ai+cn-north-1/translate>，用火山引擎账号登录
2. 完成实名认证（个人认证即可），然后开通机器翻译接口服务
3. 在 <https://console.volcengine.com/iam/keymanage> 创建 AccessKey，拿到 AccessKey ID 和 SecretAccessKey

**配置**

```console
$ transgo config set volcano.access_key_id 你的AccessKeyId
$ transgo config set volcano.secret_access_key 你的SecretAccessKey
# $ transgo config set volcano.region cn-north-1      # 默认值，一般不用改
```

**坑**

火山的语种代码是 BCP-47 风格，繁中是 `zh-Hant`，`tw` 在火山这边指的是契维语而非繁中。
transgo 已经统一过了，你只管用 `zh-TW`。
首次调用时会自动探测签名形式并记住结论，第一次慢了半拍属正常。

---

## 阿里云机器翻译

**免费额度**：通用版 100 万字符/月，每月 1 日重置，不结转。

**申请步骤**

1. 打开 <https://help.aliyun.com/zh/machine-translation/product-overview/activate-service>，开通机器翻译服务
2. 在 <https://ram.console.aliyun.com/manage/ak> 创建 AccessKey

**配置**

```console
$ transgo config set aliyun.access_key_id 你的AccessKeyId
$ transgo config set aliyun.access_key_secret 你的AccessKeySecret
```

**坑**

别和「专业版」搞混：阿里云有 `Translate`（专业版，60 元/百万字符）和 `TranslateGeneral`
（通用版，100 万字符/月免费）两个接口，transgo 走的是通用版 `TranslateGeneral`。

子账号要有 `AliyunMTFullAccess` 权限，否则报 `Forbidden.RAM`。单次请求上限 5000 字符。

---

## 百度翻译

百度翻译开放平台有两个文本翻译接口，transgo 各接了一个，共用同一套 APP ID + 密钥：

| transgo 引擎 | 接口 | 翻译模型 | 免费额度 |
|---|---|---|---|
| `baidu` | 通用翻译 API | 机器翻译 | 标准版 5 万/月，个人认证高级版 100 万/月，企业尊享版 200 万/月 |
| `baidu-llm` | 大模型文本翻译 API | 大模型翻译 | 实名认证后一次性 100 万字符测试额度，不按月重置 |

两个接口的额度分开统计：控制台「我的服务」页「剩余免费额度」旁的
「大模型翻译 / 机器翻译」切换标签，就是分别显示这两份剩余量。
超出免费额度后都是 49 元/百万字符，从账户余额扣；用量每 5 分钟更新，
费用次日结算前一天的用量，余额不足时报 `54004` 停服。账户是预付费余额制，没有自动充值。
计费按源语言字符数，汉字、字母、空格、标点、HTML 标签各算 1 个字符，调用失败不计费。

### 通用翻译 API（`baidu` 引擎）

| 版本 | 免费额度（按月重置） | 要求 | QPS | 单次上限 | 语种 |
|---|---|---|---|---|---|
| 标准版 | 5 万字符/月 | 无 | 1 | 1000 字符 | 常见语种 28 种 |
| 高级版 | 100 万字符/月 | 个人实名认证 | 10 | 6000 字符 | 常见语种 28 种 |
| 尊享版 | 200 万字符/月 | 企业认证 | 100 | 6000 字符 | 全部语种 200+ |

### 大模型文本翻译 API（`baidu-llm` 引擎）

独立接口 `ait/api/aiTextTranslate`，`model_type` 固定取 `llm`。同一个接口也能取 `nmt`
调机器翻译模型，但 transgo 的机器翻译走的是上面的通用翻译 API，不经这个接口。
QPS 10，单次上限 6000 字符，官方建议单次 2000 字符以内。
翻译指令（`reference` 参数，如「采用意译」）和术语库干预免费开放，不另行收费：

- **翻译指令**：单次用 `transgo -e baidu-llm -i "采用意译" "原文"` 指定，
  或配置 `baidu.reference` 作为默认文风；GUI 在「翻译指令」折叠区里填。
  上限 500 字符，超出报 `59002`
- **术语库**：控制台「我的术语库」上传术语对照表后，配置 `baidu.need_intervene = true`，
  译文即按术语表输出。功能本身不额外收费

鉴权两种任选：

- API Key：HTTP 头 `Authorization: Bearer <API Key>`，在控制台「API Key 管理」创建，
  配到 `baidu.api_key` 后 `baidu-llm` 就走这种
- MD5 签名：与通用翻译相同的 `appid + q + salt + 密钥` 拼接取 MD5，
  `baidu.api_key` 留空时的默认行为

**申请步骤**（两个接口通用）

1. 打开 <https://fanyi-api.baidu.com/>，用百度账号登录
2. 在「开发者信息」页拿到 APP ID 和密钥，完成个人实名认证
   （不认证只能用标准版，额度少 20 倍；大模型翻译的测试额度也要认证后才发）
3. 开通服务：<https://fanyi-api.baidu.com/choose>，通用文本翻译和大模型文本翻译分别开通
4. 在「总览 - 我的服务」确认服务类型，标准版/高级版/尊享版可自主切换

**配置**（一套凭据喂两个引擎）

```console
$ transgo config set baidu.app_id 你的APPID
$ transgo config set baidu.secret 你的密钥
$ transgo config set baidu.api_key 你的APIKey    # 可选，baidu-llm 走 Bearer 免签名
```

**余额提醒**（默认关闭）：控制台「财务总览」页，「账户余额」右侧「余额提醒 - 设置」。
可改提醒起始余额（默认 50 元），短信、邮件各最多 3 个。提醒的是账户余额，
不是免费额度用量，控制台没有免费额度用量提醒。

**坑**

- 报 `90107` = 实名认证未通过或未生效
- 报 `54001` = 签名或 token 错误，多半是 APP ID、密钥或 API Key 抄错了
- 报 `54004` = 账户余额不足，免费额度用完且余额为 0
- 报 `52003` = 服务没开通。两个接口要在控制台分别开通，只开一个只会有一个引擎能用
- 标准版/高级版只有 28 个常见语种，小语种要企业尊享版，`transgo lang -e baidu` 可看具体范围

完整报错对照见 [troubleshooting.md](troubleshooting.md)。

---

## Azure AI Translator

**免费额度**：F0 层 200 万字符/月。

**申请步骤**

1. 打开 <https://portal.azure.com/>，创建 Translator 资源（定价层选 F0）
2. 在资源的 Keys and Endpoint 里拿到 KEY 1

**配置**

```console
$ transgo config set azure.api_key 你的Key
$ transgo config set azure.region eastasia      # 多服务资源/区域资源需要
```

**坑**

多服务资源、区域资源必须填 `azure.region`，否则报 `401001`。

需要绑卡，但 F0 免费额度用完会停止服务，不会自动扣费。报 `401000` = Key 无效。

---

## Google Cloud Translation

**免费额度**：每月前 50 万字符免费，永久不过期（按月重置）。

**申请步骤**

1. 打开 <https://console.cloud.google.com/>，建一个项目
2. 启用 Cloud Translation API（<https://console.cloud.google.com/apis/library/translate.googleapis.com>）
3. 创建结算账号并绑定到项目，免费额度用的是每月 $10 赠金，50 万字符以内不会真扣钱；
   **超出免费额度会从绑定的卡实际扣费**，用量大要注意额度监控
4. 在 API 和服务 → 凭据 里创建 API 密钥

**配置**

```console
$ transgo config set google.api_key 你的APIKey
```

**坑**

必须启用 API，只建 Key 不够，否则报 `403`。`403` 也可能是 Key 限制了 IP 或接口范围，
去凭据页检查「API 限制」。国内网络可能连不上 `translation.googleapis.com`，需要自备网络环境。

---

## LLM（OpenAI 兼容）

**额度**：取决于你选的服务商。可以接 OpenAI / DeepSeek / 硅基流动 / Groq / OpenRouter，
或者本地 Ollama（完全免费、完全离线）。

**配置**

```console
$ transgo config set llm.base_url https://api.openai.com/v1
$ transgo config set llm.model gpt-4o-mini
$ transgo config set llm.api_key 你的Key
```

国内可直连的服务商示例：`https://api.deepseek.com/v1` 配 `deepseek-chat`。
本地 Ollama：`base_url = "http://localhost:11434/v1"`，`model` 填你拉下来的模型名，不需要 Key。

**坑**

`base_url` 要带 `/v1`，报 `404` 通常就是漏了这一段；`model` 名称写错同样会 `404`。
本地 Ollama、局域网自建服务（私有网段 IP）transgo 会自动免掉 Key 校验，
公网服务商则必须填 `api_key`。

**进阶：自定义系统提示词**

默认提示词要求「只输出译文、保留段落与代码块、保留原文语气」。你可以覆盖它来固定术语表或文风：

```console
$ transgo config edit
```

```toml
[llm]
system_prompt = """
你是专业翻译。术语表：
- commit -> 提交（不要译成「承诺」）
- repository -> 仓库
只输出译文，不要任何解释。
"""
```

---

## MyMemory

**免费额度**：匿名 5 千字符/天；留下邮箱可提到 5 万字符/天。

**配置**（不配也行，本来就能用）

```console
$ transgo config set mymemory.email you@example.com
```

**坑**

质量有限。它是众包翻译记忆库，译文取自其他用户提交的条目，抽到哪条看运气，
典型症状是把术语表条目当译文原样返回（`hello world` → `hello world`）。
这是数据问题，换路由解决不了。

定位是没配任何 Key 时先能用；配好任意一家后 transgo 会自动跳过它。

---

## 有道智云

**免费额度**：新账号一次性 ¥50 体验金，没有月度免费额度。余额制，余额扣完即停。

**申请步骤**

1. 打开 <https://ai.youdao.com/>，注册
2. 创建应用，把应用绑定到「文本翻译」服务（没绑定会报 `110`）
3. 拿到应用 ID（appKey）和应用密钥（appSecret）

**配置**

```console
$ transgo config set youdao.app_key 你的应用ID
$ transgo config set youdao.app_secret 你的应用密钥
```

**坑**

- 报 `110` = 应用没绑定文本翻译服务
- 报 `202` = 签名校验失败，确认密钥没错
- 报 `401` = 账户已欠费（¥50 体验金花完了）
- 有 IP 白名单机制，报 `203` 时去控制台检查

---

## 配置项速查

所有键都可用环境变量覆盖（`TRANSGO_` + 大写点分路径），环境变量优先于配置文件。

| 引擎 | 配置项 | 环境变量 |
|---|---|---|
| DeepL | `deepl.api_key` | `TRANSGO_DEEPL_API_KEY` |
| | `deepl.plan` | `TRANSGO_DEEPL_PLAN` |
| 腾讯云 | `tencent.secret_id` / `tencent.secret_key` | `TRANSGO_TENCENT_SECRET_ID` / `_SECRET_KEY` |
| 火山 | `volcano.access_key_id` / `volcano.secret_access_key` | `TRANSGO_VOLCANO_ACCESS_KEY_ID` / `_SECRET_ACCESS_KEY` |
| 阿里云 | `aliyun.access_key_id` / `aliyun.access_key_secret` | `TRANSGO_ALIYUN_ACCESS_KEY_ID` / `_SECRET_ACCESS_KEY` |
| 百度（两个接口共用） | `baidu.app_id` / `baidu.secret` / `baidu.api_key` | `TRANSGO_BAIDU_APP_ID` / `_SECRET` / `_API_KEY` |
| | `baidu.reference` / `baidu.need_intervene` | `TRANSGO_BAIDU_REFERENCE` / `_NEED_INTERVENE` |
| Azure | `azure.api_key` / `azure.region` | `TRANSGO_AZURE_API_KEY` / `TRANSGO_AZURE_REGION` |
| Google | `google.api_key` | `TRANSGO_GOOGLE_API_KEY` |
| 有道 | `youdao.app_key` / `youdao.app_secret` | `TRANSGO_YOUDAO_APP_KEY` / `_APP_SECRET` |
| LLM | `llm.api_key` / `llm.base_url` / `llm.model` | `TRANSGO_LLM_API_KEY` / `_BASE_URL` / `_MODEL` |
| 界面 | `gui.font_scale` | `TRANSGO_GUI_FONT_SCALE` |
| MyMemory | `mymemory.email` | `TRANSGO_MYMEMORY_EMAIL` |
| 全局 | `default.engine` / `default.to` / `default.from` / `default.lang` / `default.timeout_secs` | `TRANSGO_ENGINE` / `TRANSGO_TO` / `TRANSGO_FROM` / `TRANSGO_DEFAULT_LANG` |

也可以用 `TRANSGO_CONFIG` 指定另一份配置文件，方便切换多套 Key。
