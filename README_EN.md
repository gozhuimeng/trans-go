# transgo

A lightweight, fast translation tool. CLI first, Wayland / Hyprland friendly.

Only official provider APIs are used — no reverse-engineered web endpoints.

中文版：[README.md](README.md)

```console
$ transgo "知识就是力量"
Knowledge is power

$ echo "Actions speak louder than words" | transgo
事实胜于雄辩

$ transgo -t ja "你好"
こんにちは
```

---

## Features

- stdout carries the translation only; metadata always goes to stderr. Pipe-friendly and
  safe to feed straight into `wl-copy`
- 11 official translation APIs, with free quotas from 5 K characters a day to 5 M a month.
  Pick whichever suits you and fill in the keys
- Language direction is decided automatically and can be overridden manually
- Single binary. The CLI has zero runtime dependencies — no `curl`, no `wl-paste`.
  Copy the file over and it runs
- No global hotkey listening, no daemon. Your desktop environment invokes the command directly
- GUI included: `transgo gui` opens a blank window, `transgo gui --clip` pre-fills from the
  clipboard and translates once. Enter translates, Shift+Enter inserts a newline, translations
  copy with one click, ESC closes the window. The window is a singleton — invoking it again
  focuses the existing one

Why the emphasis on Wayland? For security reasons Wayland does not allow applications to
listen to global hotkeys. Tools in the pot family end up unable to register hotkeys on
Hyprland and work around it by opening a port — that is a protocol limitation, not an
implementation flaw. transgo does not fight it: the CLI itself is the integration point,
launched by `bind ... exec`, which works 100% of the time.

---

## Install

```console
$ cargo build --release          # output lands in target/release/transgo
$ cargo install --path crates/transgo-cli
```

Requires Rust 1.75+. No system development libraries needed.

After installing, run `transgo engines` once — it lists which engines still need keys.

---

## Quick start

Works without any key (falls back to the registration-free MyMemory):

```console
$ transgo "今天天气不错"
The weather is nice today
```

For larger quotas, add a key for whichever API you like. Engines without keys are skipped
automatically, no switches to flip:

```console
$ transgo config set deepl.api_key YOUR_KEY
$ transgo "今天天气不错"
It's a nice day today
```

Application steps, free quotas, overage pricing and activation requirements for every
engine: [docs_en/engines.md](docs_en/engines.md).

---

## Usage

Pass the text directly after `transgo`; the `translate` subcommand can be omitted.

```console
$ transgo "hello world"                  # direction decided automatically
$ echo "good morning" | transgo          # read from stdin, pipe friendly
$ transgo -t ja "你好"                    # pick the target language
$ transgo -f en -t zh "hello"            # pin the source language too
$ transgo -e deepl "hello"               # pick an engine
$ transgo -i "采用意译" "hello"           # translation instruction, controls style (baidu-llm)
$ transgo --json "hello"                 # full result, script friendly
$ transgo -v "hello"                     # engine, languages and timing on stderr
$ transgo -a "hello"                     # include alternative translations
```

Quoting is a shell requirement, not a transgo one: arguments without spaces need no quotes;
arguments with spaces must be quoted. Multi-word text can go unquoted
(`transgo Knowledge is power` is joined back into one sentence), but a **translation
instruction containing spaces must be quoted**: in `transgo -i 采用意译 不要直译 原文`
the part「不要直译 原文」would be treated as text to translate. The same applies to shell
metacharacters such as `$`, `!` and `*`.

### Shell completion (optional)

Tab-complete subcommands and options so you don't have to memorize them. For zsh:

```console
$ mkdir -p ~/.zfunc && transgo completions zsh > ~/.zfunc/_transgo
```

Then add one line to `~/.zshrc` before `compinit` (for oh-my-zsh users, before
`source $ZSH/oh-my-zsh.sh`), and open a new shell:

```bash
fpath=(~/.zfunc $fpath)
```

For bash use `source <(transgo completions bash)`; for fish use
`transgo completions fish > ~/.config/fish/completions/transgo.fish`.
Regenerate the script after upgrading transgo so completion knows about new options.

Full options:

```console
$ transgo translate --help
Translate text (default command; can be omitted)

Usage: transgo translate [OPTIONS] [TEXT]...

Arguments:
  [TEXT]...  Text to translate; omit or pass - to read stdin. Put text starting with - after --

Options:
  -t, --to <LANG>           Target language, e.g. zh / en / ja. auto = Chinese/English swap; third languages follow default.lang
  -f, --from <LANG>         Source language; auto-detected when omitted
  -e, --engine <ID>         Translation engine; defaults to the first configured one
  -i, --instruction <TEXT>  Translation instruction controlling style (baidu-llm only), e.g. "采用意译"
  -j, --json                Output the full result as JSON
  -a, --alternatives        Send alternative translations to stderr; stdout stays clean for pipes
  -v, --verbose             Add engine, languages and timing to stderr
  -h, --help                Print help
  -V, --version             Print version
```

### Other subcommands

| Command | What it does |
|---|---|
| `transgo engines` | List engines, free quotas and configuration status |
| `transgo engines --test` | Send one real request per configured engine to verify the keys |
| `transgo engines -v` | Also show signup URLs |
| `transgo lang` | List supported languages |
| `transgo lang -e baidu` | Show the language range of one engine |
| `transgo gui` | Open the GUI translation window (singleton: focuses the open one) |
| `transgo gui --clip` | Pre-fill from the clipboard and translate; with a window already open, sends the text there |
| `transgo config path` | Print the config file path |
| `transgo config list` | List every config key |
| `transgo config get <KEY>` | Read one key |
| `transgo config set <KEY> <VALUE>` | Write one key (empty value clears it) |
| `transgo config unset <KEY>` | Clear one key |
| `transgo completions zsh` | Generate a shell completion script (bash / fish etc. likewise) |
| `transgo config init` | Generate a commented config template |
| `transgo config edit` | Open the config in `$EDITOR` |

Exit codes: `0` success · `1` network / IO · `2` configuration or usage ·
`3` remote API error or unsupported language direction.
Scripts can use these to tell a missing key from an upstream failure.

---

## Language rules

The default is `--to auto`, with three rules:

| Source text | Target |
|---|---|
| Chinese | English |
| Latin-script text (English, French, German, Spanish, …) | Simplified Chinese |
| Everything else (Japanese, Korean, Russian, Arabic, …) | whatever `default.lang` says (default English) |

Chinese and English always swap into each other. Latin-script languages are not told apart —
they are treated as one block in the English direction. Non-Latin scripts follow
`default.lang` (`zh` / `en`, default `en`).

The decision looks at the character mix of the whole text (Chinese characters at 1/3 or more
of letter-type characters), not the first character, so Chinese text starting with an English
word such as「iPhone手机很好用」is not misjudged. Digits, punctuation and whitespace are
excluded from the count — they look the same in every language and only dilute the ratio.

Known limitation: a Chinese sentence containing a whole katakana word
(e.g.「我很喜欢カタカナ」) is judged as Japanese and follows the third-language direction.
The two cases are identical in character statistics and cannot be separated; use `-f` / `-t`
to override when it guesses wrong.

### Manual override when it guesses wrong

```console
$ transgo -t ja "你好"                    # change the target language
$ transgo -f en -t zh "hello"             # pin both sides
```

Any language code is accepted, case-insensitive, with `_`/`-` and common aliases all working
(`zh-CN` / `zh-Hans` / `chs` are equivalent). See `transgo lang` for the full list.

---

## Translation engines

All are official provider APIs. Free quotas verified 2026-09.

| Engine | Free quota | What you need |
|---|---|---|
| `deepl` | 500 K characters/month, permanent | A free signup key |
| `tencent` | 5 M characters/month | SecretId + SecretKey |
| `volcano` | First 2 M characters/month | AccessKey |
| `azure` | 2 M characters/month | A subscription + key (card required) |
| `aliyun` | 1 M characters/month | AccessKey |
| `baidu` | Standard 50 K/month; 1 M/month after identity verification | APP ID + secret |
| `baidu-llm` | One-time 1 M-character test quota after identity verification | APP ID + secret |
| `google` | 500 K characters/month, permanent | A GCP project + key (billing required) |
| `llm` | Depends on the provider | Any OpenAI-compatible API |
| `mymemory` | 5 K characters/day (50 K with an email) | No signup |
| `youdao` | No monthly quota, one-time trial credit only | AppKey + AppSecret |

Quota details and overage behavior: [docs_en/engines.md](docs_en/engines.md).
`mymemory` needs no registration and sits at the bottom of the priority order.

> `mymemory` quality is limited. It is a crowd-sourced translation memory; translations come
> from entries other users submitted and what you get is luck of the draw. The typical symptom
> is a glossary entry returned verbatim (`hello world` → `hello world`). That is a data
> problem and no router can fix it. Its role is to work before any key is configured; once any
> key is configured it is skipped automatically.

Engines without keys are skipped automatically — no switches to flip. The default engine is
the first configured one in table order above; pin one with
`transgo config set default.engine <id>`.

> Two that are not integrated: `Amazon Translate` has a free tier limited to 12 months and
> removed entirely for accounts registered after 2025-07-15; `彩云小译` offers a one-time
> trial credit rather than a monthly quota. Neither is integrated.

How to apply for a key and what to watch out for per engine:
[docs_en/engines.md](docs_en/engines.md).

---

## Configuration

Path: `~/.config/transgo/config.toml` (override with `TRANSGO_CONFIG`).

```console
$ transgo config init             # generate a commented template
$ transgo config edit             # open it in $EDITOR
$ transgo config set deepl.api_key YOUR_KEY
```

```toml
[default]
engine = ""            # leave empty to take the first configured engine by priority
to = "auto"            # auto = Chinese/English swap; third languages follow lang
from = "auto"
# lang = "en"          # third languages (ja/ko/ru/ar/…) translate to zh or en, default en
timeout_secs = 15

[ui]
lang = "zh"            # UI language: zh or en (CLI help, errors and GUI text switch together)

[deepl]
api_key = ""           # free keys end in `:fx` and route to api-free.deepl.com

[llm]
api_key = ""
base_url = "https://api.openai.com/v1"
model = "gpt-4o-mini"
# system_prompt = ""   # overrides the default prompt — handy for glossaries or tone

[gui]
font_scale = 1.0           # font scale (0.5 ~ 3.0), for laptop vs external monitor
```

The UI language (CLI help, errors and GUI text together) is set by `ui.lang`;
`transgo config set ui.lang en` switches to English.

Environment variables override the config file, handy for one-off script overrides:
`TRANSGO_DEEPL_API_KEY`, `TRANSGO_TENCENT_SECRET_ID`, `TRANSGO_LLM_MODEL`…
The template generated by `transgo config init` lists them all.

After writing, the file's permissions are tightened to `600` (it holds API keys).
Writes only persist values already present in the file — current shell environment variables
are never written into it.

---

## Desktop integration

```bash
# ~/.config/hypr/hyprland.conf

# Translate the clipboard and write the result back
bind = SUPER, T, exec, wl-paste | transgo | wl-copy

# Translate the clipboard and show a notification
bind = SUPER SHIFT, T, exec, notify-send "译文" "$(wl-paste | transgo)"
```

Since stdout carries the translation only, it also feeds straight into `rofi`, `dmenu` or
`fzf` pipelines.

More scripts, window rules and the Wayland hotkey limitation write-up:
[docs_en/hyprland.md](docs_en/hyprland.md).

---

## Troubleshooting

```console
$ transgo engines --test          # run this first: verifies every configured key
```

Error message reference and configuration debugging:
[docs_en/troubleshooting.md](docs_en/troubleshooting.md).

Problems not covered there are welcome as
[Issues](https://github.com/gozhuimeng/trans-go/issues); fixes are even more welcome as
[PRs](https://github.com/gozhuimeng/trans-go/pulls).

---

## Milestones

- [x] **M1** CLI core + 11 official translation engines + configuration management
- [x] **M2** GUI (`transgo gui` / `transgo gui --clip`)
      A standalone translation window wrapping the CLI's core library; no clipboard listening,
      no global hotkey listening
- [x] English documentation (`README_EN.md`, `docs_en/`)

---

## Documentation index

| Document | Audience | Contents |
|---|---|---|
| [docs_en/engines.md](docs_en/engines.md) | Users | How to get API keys for all 11 engines, how to configure them, what to watch out for |
| [docs_en/hyprland.md](docs_en/hyprland.md) | Users | Desktop integration, key bindings, scripts |
| [docs_en/troubleshooting.md](docs_en/troubleshooting.md) | Users | Error reference, self-checks |
| [docs_en/architecture.md](docs_en/architecture.md) | Contributors | Directory layout, engine abstraction, pitfalls, how to add an engine |

The Chinese documents are the primary versions; if the two ever disagree, trust the Chinese one.

## License

MIT

This project was developed by mimo v2.6.
