# Troubleshooting

## Step one: run this first

```console
$ transgo engines --test
```

It sends one real request per configured engine and prints `OK` / `失败 <原因>` per line
(the words are in Chinese). The vast majority of problems get pinned to a specific engine
right here.

Add `-v` to also see each engine's signup URL and language range:

```console
$ transgo engines --test -v
```

---

## By exit code

| Code | Meaning | What to do |
|---|---|---|
| `0` | Success | — |
| `1` | Network / IO problem | See [Network problems](#network-problems) |
| `2` | Configuration or usage error | See [Configuration not taking effect](#configuration-not-taking-effect) |
| `3` | Remote API error, or unsupported language direction | See the error reference below |

---

## Configuration not taking effect

**Symptom**: `transgo config set` wrote the value, but translation doesn't use it (or uses
the wrong engine).

Checks in order:

1. Environment variables beat the config file. First check nothing is shadowed by `TRANSGO_*`:
   ```console
   $ env | grep TRANSGO_
   ```
   If something is, `unset` it and retry.

2. Confirm which file was written:
   ```console
   $ transgo config path
   $ transgo config get deepl.api_key
   ```
   If `get` shows `(未设置)` but you are sure you wrote it, `TRANSGO_CONFIG` probably points
   at a different file.

3. Confirm the engine is selected:
   ```console
   $ transgo engines
   ```
   Configured engines are marked `*`. The default engine is the first configured one in
   table order.

4. To pin one engine:
   ```console
   $ transgo config set default.engine deepl
   ```

> **Note**: `transgo config set` only persists values into the config file; `TRANSGO_*`
> environment variables from the current shell are never written in. This is deliberate, so
> temporary keys can't leak into the file.

---

## Network problems

**Symptom**: `transgo: 网络请求失败: ...` with exit code `1`.

Google and Azure may be unreachable from mainland China: `google` depends on
`translation.googleapis.com` and needs a network environment of your own. Reachable without
a proxy there: `tencent` / `volcano` / `aliyun` / `baidu` / `youdao` / `deepl`.

Timeout defaults to 15 seconds and is adjustable:

```console
$ transgo config set default.timeout_secs 30
```

For proxies, transgo uses `reqwest` and honors `HTTPS_PROXY` / `HTTP_PROXY`:

```console
$ HTTPS_PROXY=http://127.0.0.1:7890 transgo "hello"
```

---

## Error reference

transgo errors look like `transgo: <engine> 返回错误 [<code>]: <message>` (the engine name
and message are in Chinese). Codes followed by a Chinese parenthetical hint are common causes
transgo already recognizes.

### General

| Symptom | Cause |
|---|---|
| `引擎「xxx」未配置` | No API key. Apply for one at [engines.md](engines.md), then `transgo config set` |
| `未知引擎「xxx」` | Wrong `-e` value; `transgo engines` lists valid ids |
| `未知语种代码「xxx」` | `transgo lang` lists valid codes |
| `引擎「xxx」不支持 a → b` | That direction is outside the engine's language range. Use another engine, or `transgo lang -e xxx` to see the range |
| `没有要翻译的文本` | No argument and stdin is a terminal. Pass `transgo "text"` or pipe it in |

### DeepL

| Code | Meaning |
|---|---|
| `403` | Invalid key, or a free key sent to the Pro endpoint (check `deepl.plan`) |
| `456` | The free quota is used up; resets next month |
| `429` | Too many requests |

### Tencent Cloud

| Code | Meaning |
|---|---|
| `AuthFailure.SignatureFailure` | The SecretKey was copied wrong |
| `AuthFailure.SecretIdNotFound` | The SecretId was copied wrong |
| `AuthFailure.SignatureExpire` | The system clock is off; fix it with `timedatectl` |
| `UnauthorizedOperation` | The sub-account lacks TMT permissions |
| `FailedOperation.UserNotRegistered` | **The MT service is not activated** — creating keys is not enough |
| `FailedOperation.NoFreeAmount` | This month's 5 M characters are used up |
| `FailedOperation.ServiceIsolate` | The account is in arrears and suspended |
| `FailedOperation.ErrorUserArea` | The account region doesn't match the requested `region` |
| `InvalidParameter` | Unsupported language direction |
| `LimitExceeded` | Quota exhausted or rate limited |

### Baidu (baidu / baidu-llm)

Both APIs share this code set; entries marked "baidu-llm only" appear only on the LLM text
translation API.

| Code | Meaning |
|---|---|
| `52001` | Request timeout; check the q parameter and language direction |
| `52002` | System error; retry |
| `52003` | Unauthorized user: wrong APP ID, or the corresponding service is not activated |
| `54000` | A required parameter is empty |
| `54001` | Signature or token error; usually a mistyped APP ID, secret or API key |
| `54003` | Rate limited (Standard tier is QPS 1) |
| `54004` | Insufficient account balance: the free quota is used up and the balance is 0 |
| `54005` | Long-text requests too frequent; retry in 3 seconds |
| `58000` | Client IP not allowed; check the server IP on the Developer Info page |
| `58001` | Unsupported translation direction (Standard / Advanced cover 28 common languages) |
| `58002` | The service is switched off; enable it on the console |
| `58003` | The IP is banned: one IP used several APP IDs in one day; unbans the next day |
| `58004` | Invalid model parameter (baidu-llm only) |
| `59002` | Translation instruction too long, cap 500 characters (baidu-llm only) |
| `59003` | Request text too long, cap 6000 characters per request |
| `59004` | QPS exceeded |
| `59005` / `59006` / `59007` | Tag-related parameter errors (baidu-llm only) |
| `90107` | Identity verification not passed or not yet effective |
| `20003` | The request content is flagged as unsafe (baidu-llm only) |

### Volcano Engine

| Code | Meaning |
|---|---|
| `-400` | Bad request parameters |
| `-415` | Unsupported language direction |
| `-429` | Too many requests |
| `InvalidCredential` / `InvalidAccessKey` | Invalid AccessKey |
| `SignatureDoesNotMatch` | Invalid SecretAccessKey |

### Alibaba Cloud

| Code | Meaning |
|---|---|
| `10005` | Unsupported language direction |
| `10008` | Text too long, cap 5000 characters per request |
| `10010` / `10013` | Service not activated or in arrears |
| `InvalidAccessKeyId.NotFound` | Invalid AccessKey ID |
| `SignatureDoesNotMatch` | Invalid AccessKey Secret |
| `Forbidden.RAM` | The RAM sub-account lacks alimt permissions |

### Azure

| Code | Meaning |
|---|---|
| `401000` | Invalid API key |
| `401001` | **`azure.region` is missing** — required for multi-service / regional resources |
| `403000` | No permission or quota exhausted |

### Google

| Code | Meaning |
|---|---|
| `400` | Invalid API key or malformed request |
| `403` | **The API key lacks permission** — usually the Cloud Translation API is not enabled |
| `429` | Quota exhausted |

### LLM

| Code | Meaning |
|---|---|
| `401` | Invalid API key |
| `402` | Insufficient balance |
| `404` | **Wrong `model` name, or `base_url` is missing `/v1`** |
| `429` | Rate limited |

### Youdao

| Code | Meaning |
|---|---|
| `108` | Invalid appKey |
| `110` | **The application is not bound to the Text Translation service** |
| `202` | Signature verification failed |
| `203` | The requesting IP is not on the whitelist |
| `207` | Replayed request (handled internally; if you see it, please file a bug) |
| `401` | The account is in arrears (the ¥50 trial credit is used up) |

### MyMemory

| Code | Meaning |
|---|---|
| `403` | Invalid language code, or the daily quota is used up |
| `429` | Rate limited |

---

## Something's wrong with the translation

### The translation is identical to the source

Usually **MyMemory corpus noise**. It is a crowd-sourced translation memory whose entries
come from other users; occasionally a glossary entry is returned verbatim
(`hello world` → `hello world`).

The real fix is switching engines; see [engines.md](engines.md) for how to get keys for the
other APIs. MyMemory's role is to work before any key is configured and it is skipped
automatically afterwards.

### The direction is reversed

The `auto` rule is "Chinese to English, everything else to Chinese", decided by character
ratio. Boundary cases flip:

| Input | Decision | Note |
|---|---|---|
| `iPhone手机很好用` | Chinese — correct | An English opening doesn't matter; the whole text counts |
| `那个 meeting 的议程定了` | Chinese — correct | A few English words don't matter |
| `The 会议 will be held tomorrow` | English — correct | Too few Chinese characters |
| `我很喜欢カタカナ` | Japanese — wrong | **Known limitation**: Chinese with katakana mixed in is statistically identical to Japanese |

Manual override:

```console
$ transgo -t en "那句话"          # explicitly English
$ transgo -f ja -t zh "日本語"    # pin the language pair
```

### Poor translation quality

Check in this order:

1. Are you on `mymemory`? `transgo -v` shows the engine actually used. If so, switch engines
2. Long text? Some engines cap a single request (Baidu 6000, Alibaba 5000) — try smaller
   chunks
3. Wrong terminology? Two options: give the `llm` engine a custom `system_prompt` with a
   glossary, or enable glossary intervention on `baidu-llm`. Both are covered in
   [engines.md](engines.md)

---

## GUI window

| Symptom | Cause and fix |
|---|---|
| Pressing the hotkey again opens no second window | Singleton design: the existing window is focused. With `--clip`, the text is sent into it and translated |
| Chinese shows as boxes | No CJK system font. Install one (on Arch: `noto-fonts-cjk`); transgo mounts system fonts automatically |
| `transgo gui` reports "启动图形界面失败" | The shell is outside a graphical session, or the compositor provides no OpenGL |
| `--clip` starts with an empty source box | The clipboard is empty, or not plain text |
| Enter does nothing | Enter with an empty source box is a no-op. Shift+Enter inserts a newline rather than translating |

## Still stuck

1. Add `-v` to see the engine and language direction actually used:
   ```console
   $ transgo -v "the text that reproduces it"
   ```
2. Add `--json` for the full response:
   ```console
   $ transgo --json "the text that reproduces it"
   ```
3. Confirm the version: `transgo --version`

The output of these three steps is enough to locate the problem — and makes a good
[Issue](https://github.com/gozhuimeng/trans-go/issues) attachment.
