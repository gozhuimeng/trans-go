# Translation Engine Setup Guide

transgo only uses official provider APIs. This document covers where to get each engine's
key, how to fill it in, and what to watch out for.

All free quotas verified 2026-09. **Signup pages get redesigned; if the steps don't match,
trust the provider's website.**

## Test status

`baidu`, `baidu-llm`, `llm` and `mymemory` have been verified with real requests in both
directions. `deepl`, `tencent`, `volcano`, `aliyun`, `azure`, `google` and `youdao` are
untested.

Problems found in use are welcome as
[Issues](https://github.com/gozhuimeng/trans-go/issues); those with a root cause identified
are even more welcome as [PRs](https://github.com/gozhuimeng/trans-go/pulls).

---

## At a glance

The table lists facts only: how the free quota is granted, what happens beyond it, and what
activation requires. Choosing one is up to you.

| Engine | Free quota | Beyond the quota | Activation requirements |
|---|---|---|---|
| `deepl` | 500 K characters/month | Service stops, no charges | A credit card issued in a country DeepL supports; domestic dual-currency or foreign-currency cards are not accepted |
| `tencent` | 5 M characters/month | Postpaid is off by default; service stops when used up | Free packs stop being issued 2026-10-01; every API is retired 2027-10-01 |
| `volcano` | First 2 M characters/month | ¥49 per million characters, billed automatically, settled at the start of the next month | Identity verification |
| `aliyun` | 1 M characters/month | ¥50 per million characters, switches to postpaid automatically and cannot be turned off | Identity verification |
| `baidu` | Standard 50 K/month; Advanced 1 M/month after personal identity verification | ¥49 per million characters, settled next day, service stops when the balance runs out | Advanced requires identity verification |
| `baidu-llm` | One-time 1 M-character test quota after identity verification | ¥49 per million characters, settled next day, service stops when the balance runs out | Identity verification; the LLM text translation service is activated separately |
| `azure` | 2 M characters/month | See Azure docs | An Azure subscription |
| `google` | 500 K characters/month | See Google docs | Billing must be bound; needs a proxy in mainland China |
| `youdao` | One-time ¥50 trial credit | Balance-based, stops when used up | None |
| `llm` | Depends on the provider | Depends on the provider | None |
| `mymemory` | 5 K characters/day (50 K with an email) | No charges involved | No signup |

Once configured, verify with one command:

```console
$ transgo engines --test
```

Each line shows `OK` / `失败 <原因>`, making it clear which keys work.

---

## DeepL

**Free quota**: 500 K characters/month, permanently. The service stops when used up and
nothing is charged.

**Activation requirements (blocked in mainland China)**: DeepL requires a VISA / MASTER
credit card issued in a country it supports. **Cards issued in mainland China are not
accepted**, dual-currency and foreign-currency cards included. This is not a country ban
(mainland China is absent from the restricted regions list) — it comes down to the issuing
bank. The steps below need a card from overseas.

**Steps**

1. Open <https://www.deepl.com/pro-api> and register (a free account works)
2. Go to API → API Keys and create a key
3. Free keys end in `:fx` — that is DeepL's official marker for free accounts

**Configuration**

```console
$ transgo config set deepl.api_key YOUR_KEY
```

**Watch out**

Free keys go to `api-free.deepl.com`, Pro keys to `api.deepl.com`. transgo picks the endpoint
from the `:fx` suffix automatically, normally nothing to do; for self-hosted gateways or
oddly shaped keys, set it explicitly:

```console
$ transgo config set deepl.plan pro        # free or pro
$ transgo config set deepl.endpoint https://your-gateway/v2/translate
```

Two errors worth remembering: `403` means the key is invalid, or a free key was sent to the
Pro endpoint (check `deepl.plan`); `456` means this month's 500 K characters are used up and
reset on the 1st of next month.

---

## Tencent Cloud TMT (retired 2027-10-01)

Announcement of 2026-09-03, "QPS adjustments and free pack sunset":

| Window | QPS |
|---|---|
| Through 2026-09-30 | No reduction |
| 2026-10-01 to 2027-01-31 | Down 5% each month from the previous month |
| 2027-02-01 to 2027-09-30 | Down 10% each month |
| From 2027-10-01 | Reduced to zero — **every API becomes unusable** |

Free packs stop being issued from 2026-10-01; packs already claimed remain usable until they
expire and are not renewed. Since packs are valid only for the month issued, the free quota
effectively ends 2026-09-30.

What follows is for people already using it and trying to last to the end. **Free quota**:
5 M characters/month for text translation. It is issued as a pack: on the day service first
opens, then on the 1st of each month, valid only for that month. **Postpaid is off by
default** — when the pack runs out the service stops; it does not convert to charges.

**Steps**

1. Open <https://console.cloud.tencent.com/tmt>, accept the service agreement and click
   "activate" — text translation and the other APIs activate together
2. Create API credentials at <https://console.cloud.tencent.com/cam/capi> to get a SecretId
   and SecretKey

**Configuration**

```console
$ transgo config set tencent.secret_id YOUR_SECRET_ID
$ transgo config set tencent.secret_key YOUR_SECRET_KEY
# $ transgo config set tencent.region ap-guangzhou     # this is the default, rarely changed
```

**Watch out**

The service must be activated first — creating keys alone is not enough. The SecretKey is
sensitive; don't commit it to git. Sub-accounts need a permission like
`QcloudTMTFullAccess`, otherwise `UnauthorizedOperation` is returned.

Error reference:

| Code | Meaning |
|---|---|
| `FailedOperation.UserNotRegistered` | The service is not activated. The key is fine — open <https://console.cloud.tencent.com/tmt> and click "activate" |
| `FailedOperation.NoFreeAmount` | This month's 5 M characters are used up; resets on the 1st |
| `FailedOperation.ServiceIsolate` | The account is in arrears and suspended |
| `FailedOperation.ErrorUserArea` | The account region doesn't match the requested `region` |
| `SignatureDoesNotMatch` | The SecretKey was copied wrong |
| `AuthFailure.SignatureExpire` | The system clock is off; fix it with `timedatectl` |

---

## Volcano Engine MT

**Free quota**: the first 2 M characters each month are free; beyond that ¥49 per million
characters.

Note that the "list price" table on the console only shows the post-quota unit price and
never mentions the free quota, which makes it easy to assume there is none. The tiering is
`[0, 2] M characters free, (2, +∞) ¥49 per million characters` — see the official
"Pricing — Machine Translation" doc.

**No hard stop.** Volcano opens with daily-metered postpaid by default: beyond the free quota
it keeps charging automatically and does not stop, and the bill only becomes visible when the
next month settles. This is the opposite of Tencent's "postpaid off by default, stops when
used up".

The cost center supports budget alerts: <https://console.volcengine.com/cost/expense> — top
up ¥1 and set the "available budget alert" to ¥1 to be notified when money is deducted.

**Steps**

1. Open <https://console.volcengine.com/ai/region:ai+cn-north-1/translate> and sign in with a
   Volcano Engine account
2. Complete identity verification (personal is enough), then activate the machine translation
   service
3. Create an AccessKey at <https://console.volcengine.com/iam/keymanage> to get an
   AccessKey ID and SecretAccessKey

**Configuration**

```console
$ transgo config set volcano.access_key_id YOUR_ACCESS_KEY_ID
$ transgo config set volcano.secret_access_key YOUR_SECRET_ACCESS_KEY
# $ transgo config set volcano.region cn-north-1      # the default, rarely changed
```

**Watch out**

Volcano's language codes are BCP-47 style: Traditional Chinese is `zh-Hant`, and `tw` on
Volcano's side means the Twi language, not Traditional Chinese. transgo normalizes all of
this — just use `zh-TW`. The first call auto-detects the signature form and remembers the
answer; being slightly slower the first time is normal.

---

## Alibaba Cloud MT

**Free quota**: 1 M characters/month on the General edition, reset on the 1st, no rollover.

**Steps**

1. Open
   <https://help.aliyun.com/zh/machine-translation/product-overview/activate-service> to
   activate the machine translation service
2. Create an AccessKey at <https://ram.console.aliyun.com/manage/ak>

**Configuration**

```console
$ transgo config set aliyun.access_key_id YOUR_ACCESS_KEY_ID
$ transgo config set aliyun.access_key_secret YOUR_ACCESS_KEY_SECRET
```

**Watch out**

Don't confuse it with the "Professional" edition: Alibaba has two APIs, `Translate`
(Professional, ¥60 per million characters) and `TranslateGeneral` (General, 1 M characters/month
free). transgo calls the General one, `TranslateGeneral`.

Sub-accounts need the `AliyunMTFullAccess` permission, otherwise `Forbidden.RAM` is returned.
Single requests cap at 5000 characters.

---

## Baidu Translate

Baidu's open platform has two text translation APIs and transgo has integrated both. They
share one APP ID + secret:

| transgo engine | API | Translation model | Free quota |
|---|---|---|---|
| `baidu` | General Translation API | Machine translation | Standard 50 K/month, Advanced (personal) 1 M/month, Premium (business) 2 M/month |
| `baidu-llm` | LLM Text Translation API | LLM translation | One-time 1 M-character test quota after identity verification, no monthly reset |

Quota is counted separately per API: on the console's "My Services" page, the
"LLM translation / machine translation" toggle next to "remaining free quota" shows each
one. Beyond the free quota both cost ¥49 per million characters, deducted from the account
balance; usage updates every 5 minutes and the previous day is settled the next day; service
stops with error `54004` when the balance runs out. The account is prepaid with no auto
top-up. Billing counts source characters — Chinese characters, letters, spaces, punctuation
and HTML tags each count as one — and failed calls are not billed.

### General Translation API (`baidu` engine)

| Tier | Free quota (monthly reset) | Required | QPS | Per request | Languages |
|---|---|---|---|---|---|
| Standard (标准版) | 50 K characters/month | None | 1 | 1000 characters | 28 common languages |
| Advanced (高级版) | 1 M characters/month | Personal identity verification | 10 | 6000 characters | 28 common languages |
| Premium (尊享版) | 2 M characters/month | Business verification | 100 | 6000 characters | All 200+ languages |

### LLM Text Translation API (`baidu-llm` engine)

The standalone `ait/api/aiTextTranslate` endpoint with `model_type` fixed to `llm`. The same
endpoint also accepts `nmt` for the machine translation model, but transgo's machine
translation goes through the General Translation API above instead. QPS 10, 6000 characters
per request, officially suggested to stay under 2000. Translation instructions (the
`reference` parameter, e.g. "采用意译") and glossary intervention are free of charge:

- **Translation instruction**: per call use
  `transgo -e baidu-llm -i "采用意译" "SOURCE"`, or set `baidu.reference` as the default
  style; in the GUI it is the collapsible "翻译指令" input. Cap 500 characters, longer
  returns `59002`
- **Glossary**: upload a term list on the console's "My Glossary" page, then set
  `baidu.need_intervene = true` — translations will follow the glossary. The feature itself
  is free

Two authentication options:

- API Key: the `Authorization: Bearer <API Key>` header, created on the console's "API Key"
  page. Put it in `baidu.api_key` and `baidu-llm` uses this method
- MD5 signature: the same `appid + q + salt + secret` concatenation and MD5 as the General
  Translation API. This is the default when `baidu.api_key` is empty

**Steps** (shared by both APIs)

1. Open <https://fanyi-api.baidu.com/> and sign in with a Baidu account
2. On the "Developer Info" page, get the APP ID and secret and complete personal identity
   verification (without it only the Standard tier is available, with 20x less quota; the
   LLM test quota is also issued only after verification)
3. Activate services at <https://fanyi-api.baidu.com/choose> — General Text Translation and
   LLM Text Translation are activated separately
4. Under "Overview - My Services", confirm the service tier; Standard / Advanced / Premium
   can be switched there

**Configuration** (one credential set feeds both engines)

```console
$ transgo config set baidu.app_id YOUR_APP_ID
$ transgo config set baidu.secret YOUR_SECRET
$ transgo config set baidu.api_key YOUR_API_KEY    # optional; baidu-llm then uses Bearer auth
```

**Balance alerts** (off by default): on the console's "Financial Overview" page, "Balance
alert - settings" sits to the right of "account balance". The alert threshold is adjustable
(defaults to ¥50), with up to 3 SMS numbers and 3 emails. It alerts on the account balance —
not free quota usage; the console has no free quota usage alert.

**Watch out**

- Error `90107` = identity verification not passed or not yet effective
- Error `54001` = signature or token error — usually a mistyped APP ID, secret or API key
- Error `54004` = insufficient account balance: the free quota is used up and the balance is 0
- Error `52003` = the service is not activated. The two APIs are activated separately;
  activating one only makes one engine work
- Standard / Advanced only cover 28 common languages; rarer languages need Premium.
  `transgo lang -e baidu` shows the exact range

Full error reference: [troubleshooting.md](troubleshooting.md).

---

## Azure AI Translator

**Free quota**: 2 M characters/month on the F0 tier.

**Steps**

1. Open <https://portal.azure.com/> and create a Translator resource (pick the F0 pricing tier)
2. Get KEY 1 from the resource's "Keys and Endpoint"

**Configuration**

```console
$ transgo config set azure.api_key YOUR_KEY
$ transgo config set azure.region eastasia      # required for multi-service / regional resources
```

**Watch out**

Multi-service and regional resources must set `azure.region`, otherwise `401001` is returned.

A card must be bound, but the service stops when the F0 free quota is used up — nothing is
charged automatically. Error `401000` = invalid key.

---

## Google Cloud Translation

**Free quota**: the first 500 K characters each month are free, permanently (monthly reset).

**Steps**

1. Open <https://console.cloud.google.com/> and create a project
2. Enable the Cloud Translation API
   (<https://console.cloud.google.com/apis/library/translate.googleapis.com>)
3. Create a billing account and bind it to the project. The free quota is covered by the
   monthly $10 credit; within the quota nothing is actually charged.
   **Beyond the free quota, charges go to the bound card in real time** — watch your usage
4. Create an API key under "APIs & Services → Credentials"

**Configuration**

```console
$ transgo config set google.api_key YOUR_API_KEY
```

**Watch out**

The API must be enabled; a key alone is not enough, otherwise `403`. `403` can also mean the
key is restricted by IP or API scope — check "API restrictions" on the credentials page.
Mainland China networks may not reach `translation.googleapis.com`; a proxy of your own is
required.

---

## LLM (OpenAI compatible)

**Quota**: depends on the provider. OpenAI / DeepSeek / SiliconFlow / Groq / OpenRouter all
work, as does a local Ollama (no cost, fully offline).

**Configuration**

```console
$ transgo config set llm.base_url https://api.openai.com/v1
$ transgo config set llm.model gpt-4o-mini
$ transgo config set llm.api_key YOUR_KEY
```

A provider reachable from mainland China as an example: `https://api.deepseek.com/v1` with
`deepseek-chat`. For a local Ollama: `base_url = "http://localhost:11434/v1"`, `model` set to
the pulled model name; no key needed.

**Watch out**

`base_url` needs the `/v1` suffix — a `404` usually means it's missing; a wrong `model` name
also gives `404`. Local Ollama and LAN-hosted services (private subnet IPs) skip the key
check automatically; public providers require `api_key`.

**Advanced: custom system prompt**

The default prompt asks for translation only, preserving paragraphs, code blocks and tone.
Override it to pin a glossary or a style:

```console
$ transgo config edit
```

```toml
[llm]
system_prompt = """
You are a professional translator. Glossary:
- commit -> 提交（不要译成「承诺」）
- repository -> 仓库
只输出译文，不要任何解释。
"""
```

---

## MyMemory

**Free quota**: 5 K characters/day anonymously; with an email, up to 50 K characters/day.

**Configuration** (optional — it works without)

```console
$ transgo config set mymemory.email you@example.com
```

**Watch out**

Quality is limited. It is a crowd-sourced translation memory whose translations come from
entries other users submitted; what you get is luck of the draw. The typical symptom is a
glossary entry returned verbatim (`hello world` → `hello world`). That is a data problem no
router can fix.

Its role is to work before any key is configured; once any key is configured, transgo skips
it automatically.

---

## Youdao Cloud

**Free quota**: a one-time ¥50 trial credit for new accounts; no monthly quota. Balance-based,
stops when the balance is used up.

**Steps**

1. Open <https://ai.youdao.com/> and register
2. Create an application and bind it to the "Text Translation" service (without binding,
   error `110` is returned)
3. Get the application ID (appKey) and application secret (appSecret)

**Configuration**

```console
$ transgo config set youdao.app_key YOUR_APP_KEY
$ transgo config set youdao.app_secret YOUR_APP_SECRET
```

**Watch out**

- Error `110` = the application is not bound to the text translation service
- Error `202` = signature verification failed; double-check the secret
- Error `401` = the account is in arrears (the ¥50 trial credit is used up)
- There is an IP whitelist; check the console on error `203`

---

## Configuration key reference

Every key can be overridden by an environment variable (`TRANSGO_` + the dotted path in upper
case); environment variables take precedence over the config file.

| Engine | Key | Environment variable |
|---|---|---|
| DeepL | `deepl.api_key` | `TRANSGO_DEEPL_API_KEY` |
| | `deepl.plan` | `TRANSGO_DEEPL_PLAN` |
| Tencent | `tencent.secret_id` / `tencent.secret_key` | `TRANSGO_TENCENT_SECRET_ID` / `_SECRET_KEY` |
| Volcano | `volcano.access_key_id` / `volcano.secret_access_key` | `TRANSGO_VOLCANO_ACCESS_KEY_ID` / `_SECRET_ACCESS_KEY` |
| Alibaba | `aliyun.access_key_id` / `aliyun.access_key_secret` | `TRANSGO_ALIYUN_ACCESS_KEY_ID` / `_SECRET_ACCESS_KEY` |
| Baidu (both APIs) | `baidu.app_id` / `baidu.secret` / `baidu.api_key` | `TRANSGO_BAIDU_APP_ID` / `_SECRET` / `_API_KEY` |
| | `baidu.reference` / `baidu.need_intervene` | `TRANSGO_BAIDU_REFERENCE` / `_NEED_INTERVENE` |
| Azure | `azure.api_key` / `azure.region` | `TRANSGO_AZURE_API_KEY` / `TRANSGO_AZURE_REGION` |
| Google | `google.api_key` | `TRANSGO_GOOGLE_API_KEY` |
| Youdao | `youdao.app_key` / `youdao.app_secret` | `TRANSGO_YOUDAO_APP_KEY` / `_APP_SECRET` |
| LLM | `llm.api_key` / `llm.base_url` / `llm.model` | `TRANSGO_LLM_API_KEY` / `_BASE_URL` / `_MODEL` |
| GUI | `gui.font_scale` | `TRANSGO_GUI_FONT_SCALE` |
| MyMemory | `mymemory.email` | `TRANSGO_MYMEMORY_EMAIL` |
| Global | `default.engine` / `default.to` / `default.from` / `default.lang` / `default.timeout_secs` | `TRANSGO_ENGINE` / `TRANSGO_TO` / `TRANSGO_FROM` / `TRANSGO_DEFAULT_LANG` |

`TRANSGO_CONFIG` points to a different config file entirely, handy for switching between key
sets.
