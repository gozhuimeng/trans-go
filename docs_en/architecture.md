# Implementation Notes

For people who want to modify the code or add an engine. End users should see the
[README](../README_EN.md).

---

## Directory layout

```
crates/
├── transgo-core/           Engine abstraction layer, shared by CLI and GUI
│   └── src/
│       ├── lib.rs          HTTP client, timeouts, shared utilities
│       ├── lang.rs         Unified language enum + per-provider code mapping
│       ├── detect.rs       "Is this Chinese?" decision + target language rules
│       ├── types.rs        Request / Translation
│       ├── config.rs       config.toml I/O, env overrides, dotted-path access
│       ├── error.rs        Unified error type + exit code mapping
│       └── engine/
│           ├── mod.rs      Engine trait, registry, default engine selection
│           ├── util.rs     Response capture, error parsing, HTML unescaping, etc.
│           ├── mymemory.rs     Registration-free
│           ├── deepl.rs        DeepL (500 K free/month)
│           ├── tencent.rs      Tencent TMT (TC3-HMAC-SHA256)
│           ├── volcano.rs      Volcano Engine (Volcengine V4 signing)
│           ├── aliyun.rs       Alibaba Cloud (POP-RPC HMAC-SHA1)
│           ├── baidu.rs        Baidu General Translation (MD5 signature)
│           ├── baidu_llm.rs    Baidu LLM Text Translation (Bearer API key or MD5 signature)
│           ├── azure.rs        Azure AI Translator
│           ├── google.rs       Google Cloud Translation v2
│           ├── youdao.rs       Youdao Cloud (SHA-256 hex signature)
│           └── openai.rs       Any OpenAI-compatible API
├── transgo-cli/            The transgo executable
│   └── src/main.rs         Argument parsing, subcommands, output formatting
└── transgo-gui/            GUI (eframe/egui), wrapping the core library
    └── src/lib.rs          Translation window, singleton messages, worker thread, system fonts
```

Around 4600 lines with 29 unit tests.

---

## Layers

```
                ┌─────────────┐   ┌─────────────┐
                │ transgo-cli │   │ transgo-gui │
                └──────┬──────┘   └──────┬──────┘
                       │                 │
                       └────────┬────────┘
                                ▼
                        ┌──────────────┐
                        │ transgo-core │
                        └──────────────┘
```

`transgo-core` takes a `Request { text, from, to }` and returns a `Translation`. It has no
idea whether the caller is the CLI or the GUI, and doesn't care who decided the language
direction. The GUI reuses it as-is; core never changes for GUI's sake.

---

## The Engine trait

```rust
#[async_trait]
pub trait Engine: Send + Sync {
    fn id(&self) -> &'static str;              // stable id, used by -e and the config
    fn name(&self) -> &'static str;            // display name
    fn quota(&self) -> &'static str;           // free quota description
    fn signup(&self) -> &'static str;          // signup URL
    fn configured(&self) -> bool;              // are the keys in place
    fn languages(&self) -> Option<&'static [Lang]>;  // None = no restriction

    fn supports(&self, req: &Request) -> bool { ... }  // default implementation
    async fn translate(&self, req: &Request) -> Result<Translation>;
}
```

`languages()` returning `None` means no whitelist restriction (LLM, MyMemory, Google,
Alibaba, Volcano, Youdao). Engines returning `Some(&[...])` genuinely have constrained
ranges: DeepL 23 languages, Azure 29, Tencent 18, Baidu 22 (`baidu` and `baidu-llm` share
that whitelist).

`pick_default()` selection order:

1. If `default.engine` is set in the config, use it
2. Otherwise walk `engine::ORDER` and take the first `configured() && supports(req)`
3. If none qualify, fall back to the first `supports(req)` one (it will then error with
   "not configured" and ask for keys)

`ORDER` is sorted by "reachable from mainland China + free quota":

```rust
pub const ORDER: &[&str] = &[
    "deepl", "tencent", "volcano", "aliyun", "baidu", "baidu-llm", "azure", "google", "youdao",
    "llm", "mymemory",
];
```

`mymemory` sits at the end: it needs no registration, which guarantees something works even
with no keys configured at all.

---

## The unified language layer

Provider language codes differ wildly; one concept takes many spellings:

| | Simplified Chinese | Traditional Chinese | Japanese | Korean |
|---|---|---|---|---|
| **transgo** | `zh` | `zh-TW` | `ja` | `ko` |
| DeepL | `ZH` | `ZH-HANT` | `JA` | `KO` |
| Baidu | `zh` | **`cht`** | **`jp`** | **`kor`** |
| Youdao | **`zh-CHS`** | **`zh-CHT`** | `ja` | `ko` |
| Volcano | `zh` | **`zh-Hant`** | `ja` | `ko` |
| Azure | **`zh-Hans`** | `zh-Hant` | `ja` | `ko` |
| Tencent | `zh` | `zh-TW` | `ja` | `ko` |
| Google | `zh-CN` | `zh-TW` | `ja` | `ko` |
| Alibaba | `zh` | `zh-tw` | `ja` | `ko` |

How it is handled:

- `lang.rs` defines one set of generic codes (ISO 639-1 style)
- Each engine writes its own `fn code(l: Lang) -> &'static str` mapping. No centralized map:
  every file is self-contained and changing one provider can't break another
- `Lang::from_code()` does lenient parsing on the input side — case, `_`/`-`, and provider
  aliases (`zh-CN` / `zh-Hans` / `chs` / `cht` / `jp` / `kr` / `iw`…) all resolve

---

## Target language decision

Three rules: Chinese and English swap into each other; Latin-script text swaps in the English
direction; non-Latin scripts follow `default.lang`.

The judgment lives in `detect::is_chinese()` and `detect::auto_target()`. Design points:

- It looks at the character mix of the whole text (Chinese characters at 1/3 or more of
  letter-type characters), not the first character — otherwise Chinese starting with an
  English word, like「iPhone手机很好用」, gets misjudged
- Digits, punctuation and whitespace are excluded. They look identical in every language;
  including them only dilutes the ratio and drags borderline cases around the threshold
- Latin-script languages are deliberately not distinguished: characters can't separate
  English from French from German, so they swap as one block in the English direction
- Non-Latin scripts (Japanese, Korean, Russian, Arabic, …) follow `default.lang`
  (`zh` / `en`, default `en`)

`detect::detect()` is a different thing: it exists only for **display** (filling the `from`
field of `-v` / `--json` when the engine doesn't report a detected source language). It does
not affect the translation direction.

### Known limitation

A Chinese sentence containing a whole katakana word (「我很喜欢カタカナ」) is judged as
Japanese and follows the third-language direction. Statistically the two are identical
(roughly half Chinese characters, half kana) and cannot be separated — override with
`-f` / `-t` when it guesses wrong. Likewise French, German and Spanish are indistinguishable
from English by characters alone and all swap in the English direction.

---

## Adding a new engine

1. Create `foo.rs` under `engine/` and implement the `Engine` trait
2. If its language codes differ from the generic ones, write `fn code(l: Lang) -> &'static str`
3. In `engine/mod.rs`, add `mod foo;` and a branch in the `build_all()` match
4. Add the id to `ORDER` (this decides its position in default selection)
5. Add the config struct in `config.rs` and its environment override in `apply_env()`
6. Add a commented config section to `TEMPLATE` in `main.rs`
7. For errors, follow `engine/util.rs` (`api_err()` / `message_of()`) and translate each
   provider's error codes into Chinese hints (users will thank you)

Signature-based engines (Tencent TC3, Alibaba POP-RPC, Volcano V4, Baidu MD5, Youdao
SHA-256) keep their implementations in their own files. There is no shared signing library —
the algorithms differ so much that extracting one would only make them harder to read.

---

## Pitfalls (hard-won)

These were found the hard way in testing. Don't regress them.

### MyMemory's main translation field

Take `responseData.translatedText`. `matches[]` is the translation-memory hit list; one
source segment can hit both `你好` and `您好`, and the ordering drifts between requests.

### MyMemory source language must be `Autodetect`

With an unknown source language, never pass a local guess off as a specific one. Measured
with `zh-CN -> en-GB`:

| Input | Specific source given | `Autodetect` |
|---|---|---|
| `今天天气不错` | `today's Minnesota ideas` (wrong) | `It's a nice day today` (right) |
| `知识就是力量` | `Knowledge is power` (right) | `Knowledge is power` (right) |

A specific source triggers translation-memory lookup, whose quality is a lottery; `Autodetect`
takes the machine translation path, which is steadier.

There are counterexamples the other way (`hello world` under `Autodetect` returns
`hello world`, under `en` returns `你好世界`), so no route is stably better. It's a data
quality problem and no routing can solve it.

### Never present a local guess as the source language

Sending French text to MyMemory labeled `en` makes it return the input untranslated. Latin
letters span dozens of languages (English, French, German, Spanish…); character classes
can't tell them apart, and a wrong guess is far worse than none.

### SIGPIPE

Rust's `std` ignores SIGPIPE at startup, which makes `transgo lang | head` panic outright
(`failed printing to stdout: Broken pipe`).

A CLI must restore `SIG_DFL` at the top of `main`. Unix pipe semantics say "when the read
end closes, exit quietly".

### Volcano Engine V4 signing: the `StringToSign`

Three places in the official docs specify the second line as the full `YYYYMMDDTHHMMSSZ`,
but the sample code in `volc-openapi-demos` names the variable `date`, which reads like the
short `YYYYMMDD`.

Because it can't be verified offline, the implementation retries once with the other form on
the first signature error and records the answer in an `AtomicU8`, after which calls go
straight to the right form. **If you have a Volcano key and verify it, delete the probing
logic.**

### Baidu LLM Text Translation: `salt`

The official docs describe `salt` as "a string of letters or digits" — true for the General
Translation API. In the LLM API's JSON body `salt` must be an int64 number: a string gives
`53001` (parse json body error: readUint64) and anything above the int64 limit also gives
`53001` (ReadInt64: overflow). The signature concatenates its decimal form, same as the
General API. `baidu_llm.rs` takes the low 63 bits of a UUID for exactly this reason.

### LLM prompts must use full English language names

`name_en()` used to emit enum identifiers (`Zh`, `Ja`) via `stringify!`. Large chat models
can guess what those mean; small ones can't: a 1.8B-class translation model given
`target language: Zh` drifts the target to random languages like Hindi or Turkish, even when
explicitly directed. With full names like `Simplified Chinese` it becomes stable immediately.
The same class of model occasionally echoes the prompt separator `---`; the output cleanup in
`openai.rs` strips it.

### Error code types are inconsistent across providers

- Baidu's `error_code` is a string in the examples and an integer in the parameter table
- Alibaba's `Code` is the integer `200` on success and a string in gateway errors
- Youdao's `errorCode` is a string (`"0"` means success)

Parsing must tolerate both types.

### CJK alignment in the terminal

Chinese characters occupy two columns; aligning tables with `chars().count()` misaligns
them. `main.rs` has `dwidth()` / `pad()` to pad by display width.

### clap's positional-argument vs subcommand ambiguity

clap can't hold both a greedy positional (`[TEXT]...`) and subcommands in one command without
ambiguity. `main.rs::normalize_argv()` does a plain string pre-scan and inserts the default
`translate` subcommand, making `transgo "hello"` equivalent to `transgo translate "hello"`.
The check is deterministic and doesn't rely on clap heuristics.

The text argument deliberately avoids `trailing_var_arg` / `allow_hyphen_values`: with them,
options after the first positional get swallowed as text to translate (`transgo "hello" -v`
once produced `你好-v`). The cost is that text starting with `-` goes after `--`
(`transgo -- -v`), while a bare `-` remains the stdin marker. Multi-word text needs no
quotes — separate arguments are joined back with spaces.

---

## Tests

```console
$ cargo test
```

29 unit tests focused on the parts that break easily:

- `detect.rs`: boundaries of the Chinese judgment (English-opened Chinese, Chinese with
  English words, digits/punctuation dilution, Japanese and Korean)
- `config.rs`: dotted-path read/write, scalar parsing
- `youdao.rs`: UTF-16 code unit semantics of `truncate()` (it must match Youdao's official
  JS SDK, not character counts)
- `tencent.rs`: SHA-256 output stability
- `openai.rs`: stripping the model's embellishments — wrapping quotes (keeping inner ones)
  and echoed separators
- `util.rs`: HTML entity unescape order (`&amp;` must come last)

Request construction for signature-based engines can't be unit-tested offline (real keys
required); use `transgo engines --test` for integration verification.

---

## Pre-release checklist

```console
$ cargo test
$ cargo clippy --locked --all-targets -- -D warnings    # same as CI: local pass = CI pass
$ cargo build --release
$ transgo engines            # confirm no behavior changes from warnings
$ transgo lang | head -3     # confirm the SIGPIPE fix is intact
```
