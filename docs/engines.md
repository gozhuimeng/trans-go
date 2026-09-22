# 翻译引擎申请与配置指南

transgo 只接提供商官方接口。本文档告诉你每个引擎的 Key 去哪申请、怎么填、有什么坑。

全部免费额度核实于 2026-09。**申请页面会改版，若步骤对不上以官网为准。**

---

## 先说怎么选

| 你的需求 | 用什么 |
|---|---|
| 只想快点用起来 | `mymemory`：免注册，transgo 一开始就在用 |
| 日常够用、不想折腾 | `deepl`：免费 50 万字符/月，质量最好，注册即用 |
| 用量大 | `tencent`：免费 500 万字符/月，国内额度最大方 |
| 中文场景多 | `baidu`（高级版）或 `aliyun` |
| 想要可控的术语表 / 文风 | `llm`：改系统提示词就能固定术语、语气 |
| 完全离线内网 | `llm` 指向本地 Ollama |

推荐 `deepl` + `tencent` 搭着用，日常场景全覆盖。

配好之后跑一次确认：

```console
$ transgo engines --test
```

每行会显示 `OK` / `失败 <原因>`，一眼看出哪把 Key 是好的。

---

## DeepL（质量最好）

**免费额度**：50 万字符/月，永久有效，用完直接停服，不会扣费。

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

## 腾讯云机器翻译（额度最大方）

**免费额度**：500 万字符/月，国内几家里最大方。

**申请步骤**

1. 打开 <https://cloud.tencent.com/document/product/551/40566>，用腾讯云账号登录
2. 开通机器翻译（TMT）服务，没开通的话调用会报 `ResourceUnavailable.ServiceNotOpen`
3. 去 <https://console.cloud.tencent.com/cam/capi> 创建 API 密钥，拿到 SecretId 和 SecretKey

**配置**

```console
$ transgo config set tencent.secret_id 你的SecretId
$ transgo config set tencent.secret_key 你的SecretKey
# $ transgo config set tencent.region ap-guangzhou     # 默认就是这个，一般不用改
```

**坑**

必须先开通服务，只建密钥不够。SecretKey 是敏感信息，别提交进 git。
子账号要有 `QcloudTMTFullAccess` 之类的权限，否则报 `UnauthorizedOperation`。

报 `SignatureDoesNotMatch` 多半是 SecretKey 抄错了，`AuthFailure.SignatureExpire` 则是系统
时钟不准，`timedatectl` 校准一下。

---

## 火山引擎机器翻译

**免费额度**：200 万字符/月。

**申请步骤**

1. 打开 <https://console.volcengine.com/ai/region:ai+cn-north-1/translate>，用火山引擎账号登录
2. 开通机器翻译服务
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

**免费额度**：

| 版本 | 免费额度 | 要求 | QPS |
|---|---|---|---|
| 标准版 | 5 万字符/月 | 无 | 1 |
| **高级版** | **100 万字符/月** | **个人实名认证** | 10 |
| 尊享版 | 200 万字符/月 | 企业认证 | 100 |

**强烈建议做实名认证切到高级版**，否则 5 万字符/月基本干不了什么。

**申请步骤**

1. 打开 <https://fanyi-api.baidu.com/api/trans/product/desktop>，用百度账号登录
2. 注册成为开发者，完成个人实名认证（想用标准版可跳过，但额度少 20 倍）
3. 开通「通用翻译 API」，在控制台确认是高级版
4. 拿到 APP ID 和密钥

**配置**

```console
$ transgo config set baidu.app_id 你的APPID
$ transgo config set baidu.secret 你的密钥
```

**坑**

- 报 `90107` = 实名认证未通过或未生效
- 报 `54001` = 签名错误，99% 是 APP ID 或密钥抄错了
- 报 `54004` = 余额不足。超出免费额度会按 49 元/百万字符计费，建议去「我的服务 - 通用翻译」开启用量提醒
- 标准版/高级版只有 28 个常见语种，小语种要企业尊享版，`transgo lang -e baidu` 可看具体范围

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
3. 创建结算账号并绑定到项目，免费额度用的是每月 $10 赠金，正常用量不会真扣钱
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

国内推荐 `https://api.deepseek.com/v1` + `deepseek-chat`，便宜且中文好。
本地 Ollama：`base_url = "http://localhost:11434/v1"`，`model` 填你拉下来的模型名，不需要 Key。

**坑**

`base_url` 要带 `/v1`，报 `404` 通常就是漏了这一段；`model` 名称写错同样会 `404`。
走本地 Ollama 时 transgo 会自动免掉 Key 校验（识别 `localhost` / `127.0.0.1`）。

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

## MyMemory（免注册兜底）

**免费额度**：匿名 5 千字符/天；留下邮箱可提到 5 万字符/天。

**配置**（不配也行，本来就能用）

```console
$ transgo config set mymemory.email you@example.com
```

**坑**

质量有限。它是众包翻译记忆库，译文取自其他用户提交的条目，抽到哪条看运气，
典型症状是把术语表条目当译文原样返回（`hello world` → `hello world`）。
这是数据问题，换路由解决不了。

定位是没配任何 Key 时先能用；配好 `deepl` / `tencent` 后 transgo 会自动跳过它。

---

## 有道智云

**免费额度**：新账号只有一次性 ¥50 体验金，没有月度免费额度，十家里最弱。
加它纯粹是多条退路，不建议主用。

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
| 百度 | `baidu.app_id` / `baidu.secret` | `TRANSGO_BAIDU_APP_ID` / `TRANSGO_BAIDU_SECRET` |
| Azure | `azure.api_key` / `azure.region` | `TRANSGO_AZURE_API_KEY` / `TRANSGO_AZURE_REGION` |
| Google | `google.api_key` | `TRANSGO_GOOGLE_API_KEY` |
| 有道 | `youdao.app_key` / `youdao.app_secret` | `TRANSGO_YOUDAO_APP_KEY` / `_APP_SECRET` |
| LLM | `llm.api_key` / `llm.base_url` / `llm.model` | `TRANSGO_LLM_API_KEY` / `_BASE_URL` / `_MODEL` |
| MyMemory | `mymemory.email` | `TRANSGO_MYMEMORY_EMAIL` |
| 全局 | `default.engine` / `default.to` / `default.from` / `default.timeout_secs` | `TRANSGO_ENGINE` / `TRANSGO_TO` / `TRANSGO_FROM` |

也可以用 `TRANSGO_CONFIG` 指定另一份配置文件，方便切换多套 Key。
