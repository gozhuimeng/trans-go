# 常见问题与故障排查

## 第一步：先跑这个

```console
$ transgo engines --test
```

它会对每个已配置的引擎实发一次请求，逐行显示 `OK` / `失败 <原因>`。
绝大多数问题在这一步就能定位到具体引擎。

再加 `-v` 可以看到每个引擎的申请地址和语种范围：

```console
$ transgo engines --test -v
```

---

## 按退出码排查

| 退出码 | 含义 | 怎么办 |
|---|---|---|
| `0` | 成功 | — |
| `1` | 网络 / IO 问题 | 看 [网络问题](#网络问题) |
| `2` | 配置或用法错误 | 看 [配置不生效](#配置不生效) |
| `3` | 远端接口报错 | 看下面的错误信息对照表 |

---

## 配置不生效

**症状**：`transgo config set` 写进去了，但翻译时没用上 / 用错了引擎。

排查顺序：

1. 环境变量优先于配置文件，先看有没有被 `TRANSGO_*` 盖掉：
   ```console
   $ env | grep TRANSGO_
   ```
   有的话 `unset` 掉再试。

2. 确认写进的是哪份文件：
   ```console
   $ transgo config path
   $ transgo config get deepl.api_key
   ```
   如果 `get` 显示 `(未设置)` 但你确定写过，多半是被 `TRANSGO_CONFIG` 指到了别的文件。

3. 确认引擎被选中了：
   ```console
   $ transgo engines
   ```
   带 `*` 的是已配置的。默认引擎按表顺序取第一个已配置项。

4. 想固定用某个引擎：
   ```console
   $ transgo config set default.engine deepl
   ```

> **注意**：`transgo config set` 只持久化配置文件里的值，不会把当前 shell 的
> `TRANSGO_*` 环境变量一起写进去。这是有意为之，避免把临时的 Key 泄漏进配置文件。

---

## 网络问题

**症状**：`transgo: 网络请求失败: ...`，退出码 `1`。

国内访问 Google / Azure 可能不通：`google` 依赖 `translation.googleapis.com`，需要自备网络环境，
国内直连友好的是 `tencent` / `volcano` / `aliyun` / `baidu` / `youdao` / `deepl`。

超时默认 15 秒，可调：

```console
$ transgo config set default.timeout_secs 30
```

代理方面，transgo 走 `reqwest`，认 `HTTPS_PROXY` / `HTTP_PROXY` 环境变量：

```console
$ HTTPS_PROXY=http://127.0.0.1:7890 transgo "hello"
```

---

## 错误信息对照

transgo 的报错形如 `transgo: <引擎> 返回错误 [<错误码>]: <说明>`。错误码后面带中文括号提示的，
是已经识别出来的常见原因。

### 通用

| 症状 | 原因 |
|---|---|
| `引擎「xxx」未配置` | 没填 API Key。见 [docs/engines.md](engines.md) 申请后 `transgo config set` |
| `未知引擎「xxx」` | `-e` 参数写错了，`transgo engines` 看可用 id |
| `未知语种代码「xxx」` | `transgo lang` 看可用代码 |
| `引擎「xxx」不支持 a → b` | 该引擎的语种范围不含这个方向。换个引擎，或 `transgo lang -e xxx` 看范围 |
| `没有要翻译的文本` | 既没给参数、stdin 也是终端。用 `transgo "文本"` 或管道传入 |

### DeepL

| 错误码 | 含义 |
|---|---|
| `403` | Key 无效，或免费 Key 被发到了 Pro 接口（检查 `deepl.plan`） |
| `456` | 免费额度用完，下月重置 |
| `429` | 请求过于频繁 |

### 腾讯云

| 错误码 | 含义 |
|---|---|
| `AuthFailure.SignatureFailure` | SecretKey 抄错了 |
| `AuthFailure.SecretIdNotFound` | SecretId 抄错了 |
| `AuthFailure.SignatureExpire` | 系统时钟不准，`timedatectl` 校准 |
| `UnauthorizedOperation` | 子账号无 TMT 权限 |
| `FailedOperation.UserNotRegistered` | **机器翻译服务没开通**，只建了密钥不够 |
| `FailedOperation.NoFreeAmount` | 本月 500 万字符用完了 |
| `FailedOperation.ServiceIsolate` | 账号欠费停服 |
| `FailedOperation.ErrorUserArea` | 账号区域和请求的 `region` 对不上 |
| `InvalidParameter` | 语种方向不支持 |
| `LimitExceeded` | 配额耗尽或触发限频 |

### 百度（baidu / baidu-llm）

两个接口共用这套错误码，标注「仅 baidu-llm」的只有大模型文本翻译会出现。

| 错误码 | 含义 |
|---|---|
| `52001` | 请求超时，检查 q 参数与语种方向 |
| `52002` | 系统错误，重试 |
| `52003` | 未授权用户，APP ID 不对，或对应接口的服务没开通 |
| `54000` | 必填参数为空 |
| `54001` | 签名或 token 错误，多半是 APP ID、密钥或 API Key 抄错了 |
| `54003` | 访问频率受限（标准版 QPS 只有 1） |
| `54004` | 账户余额不足，免费额度用完且余额为 0 |
| `54005` | 长文本请求过频，3 秒后重试 |
| `58000` | 客户端 IP 非法，检查开发者信息页的服务器 IP |
| `58001` | 译文语言方向不支持（标准版/高级版只有 28 个常见语种） |
| `58002` | 服务已关闭，去控制台开启 |
| `58003` | IP 被封禁，同一 IP 当日用了多个 APP ID，次日解封 |
| `58004` | 模型参数错误（仅 baidu-llm） |
| `59002` | 翻译指令过长，上限 500 字符（仅 baidu-llm） |
| `59003` | 请求文本过长，单次上限 6000 字符 |
| `59004` | QPS 超限 |
| `59005` / `59006` / `59007` | 标签相关参数错误（仅 baidu-llm） |
| `90107` | 实名认证未通过或未生效 |
| `20003` | 请求内容存在安全风险（仅 baidu-llm） |

### 火山引擎

| 错误码 | 含义 |
|---|---|
| `-400` | 请求参数错误 |
| `-415` | 语种方向不支持 |
| `-429` | 请求过于频繁 |
| `InvalidCredential` / `InvalidAccessKey` | AccessKey 无效 |
| `SignatureDoesNotMatch` | SecretAccessKey 无效 |

### 阿里云

| 错误码 | 含义 |
|---|---|
| `10005` | 语种方向不支持 |
| `10008` | 文本过长，单次上限 5000 字符 |
| `10010` / `10013` | 服务未开通或欠费 |
| `InvalidAccessKeyId.NotFound` | AccessKey ID 无效 |
| `SignatureDoesNotMatch` | AccessKey Secret 无效 |
| `Forbidden.RAM` | RAM 子账号无 alimt 权限 |

### Azure

| 错误码 | 含义 |
|---|---|
| `401000` | API Key 无效 |
| `401001` | **缺少 `azure.region`**，多服务资源 / 区域资源必须填 |
| `403000` | 无权限或配额耗尽 |

### Google

| 错误码 | 含义 |
|---|---|
| `400` | API Key 无效或请求格式有问题 |
| `403` | **API Key 无权限**，多半是没启用 Cloud Translation API |
| `429` | 配额已用尽 |

### LLM

| 错误码 | 含义 |
|---|---|
| `401` | API Key 无效 |
| `402` | 余额不足 |
| `404` | **`model` 名称不对，或 `base_url` 漏了 `/v1`** |
| `429` | 速率受限 |

### 有道

| 错误码 | 含义 |
|---|---|
| `108` | appKey 无效 |
| `110` | **应用没绑定「文本翻译」服务** |
| `202` | 签名校验失败 |
| `203` | 访问 IP 不在白名单 |
| `207` | 重放请求（内部已规避，见到请报 bug） |
| `401` | 账户已欠费（¥50 体验金花完了） |

### MyMemory

| 错误码 | 含义 |
|---|---|
| `403` | 语种代码非法，或当日额度用完 |
| `429` | 频率受限 |

---

## 译文不对劲

### 译文和原文一模一样

多半是 **MyMemory 的语料噪声**。它是众包翻译记忆库，译文取自其他用户提交的条目，
偶尔会把术语表条目当译文原样返回（`hello world` → `hello world`）。

根本解决办法是换引擎，各家接口的额度、计费和申请步骤见 [docs/engines.md](engines.md)。

MyMemory 的定位就是「没配任何 Key 时先能用」，配好后会被自动跳过。

### 方向译反了

`auto` 的规则是「中英互转，第三方语言翻向 `default.lang`」，靠字符类别判定。
边界情况会翻：

| 输入 | 判定 | 说明 |
|---|---|---|
| `iPhone手机很好用` | 判为中文，正确 | 英文开头不影响，看整段比例 |
| `那个 meeting 的议程定了` | 判为中文，正确 | 夹几个英文词不影响 |
| `The 会议 will be held tomorrow` | 判为英文方向，正确 | 汉字占比太低 |
| `我很喜欢カタカナ` | 判为日文，误判 | **已知局限**：夹了片假名词的中文和日文，统计上区分不了，会走第三方方向 |

手动覆盖：

```console
$ transgo -t en "那句话"          # 明确要英文
$ transgo -f ja -t zh "日本語"    # 明确语种对
```

### 译文质量差

按这个顺序排查：

1. 是不是在用 `mymemory`？`transgo -v` 看实际用的引擎。是的话换成其他引擎
2. 长文本？有些引擎单次有字符上限（百度 6000、阿里云 5000），切小段试
3. 术语不对？`llm` 引擎可自定义 `system_prompt` 固定术语表，`baidu-llm` 可开术语库干预，
   两条路都见 [docs/engines.md](engines.md)

---

## GUI 窗口

| 现象 | 原因和办法 |
|---|---|
| 再按快捷键没弹出第二个窗口 | 单例设计：聚焦已有窗口。`--clip` 的文本会送进窗口直接翻 |
| 中文显示成方框 | 系统缺中文字体。装一个（Arch 下 `noto-fonts-cjk`），transgo 会从系统字体里自动挂载 |
| `transgo gui` 报「启动图形界面失败」 | 当前 shell 不在图形会话里，或合成器没给 OpenGL |
| `--clip` 启动后源文本是空的 | 剪贴板是空的，或里面不是纯文本 |
| 回车没反应 | 源文本为空时回车不做任何事。Shift + 回车是换行，不是翻译 |

## 还是不行

1. 加 `-v` 看实际用的引擎和语种方向：
   ```console
   $ transgo -v "复现问题的文本"
   ```
2. 加 `--json` 看完整返回：
   ```console
   $ transgo --json "复现问题的文本"
   ```
3. 确认版本：`transgo --version`

把这三步的输出贴出来，就足够定位问题了。
