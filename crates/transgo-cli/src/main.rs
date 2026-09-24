//! `transgo` 命令行客户端。
//!
//! 典型用法：
//! ```text
//! transgo "hello world"                  # 中译英 / 英译中自动决策
//! echo "good morning" | transgo          # 管道
//! transgo -t ja "你好"                    # 指定目标语
//! transgo -e deepl "hello"               # 指定引擎
//! transgo --json "hello"                 # 完整结果，便于脚本处理
//! transgo engines --test                 # 看看哪些引擎可用
//! transgo config set deepl.api_key xxx   # 配置 API Key
//! ```

use std::ffi::OsString;
use std::io::{IsTerminal, Read};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};

use transgo_core::config::config_path;
use transgo_core::engine::{self, Engine};
use transgo_core::i18n::{self, s, UiLang};
use transgo_core::{Config, Error, Lang, Request};

#[tokio::main]
async fn main() -> ExitCode {
    restore_sigpipe();
    // 界面语言必须在解析之前定好：--help 和解析报错都要按它出文案
    if let Err(e) = init_ui_lang() {
        eprintln!("transgo: {e}");
        return ExitCode::from(2);
    }
    let matches = localize_cmd(<Cli as CommandFactory>::command()).get_matches_from(normalize_argv());
    let cli = match <Cli as FromArgMatches>::from_arg_matches(&matches) {
        Ok(cli) => cli,
        Err(e) => e.exit(),
    };
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("transgo: {e}");
            ExitCode::from(e.exit_code().clamp(0, 255) as u8)
        }
    }
}

/// 读取 `ui.lang` 并设定全局界面语言。取值非法时如实报错退出。
fn init_ui_lang() -> Result<(), String> {
    let cfg = Config::load().map_err(|e| e.to_string())?;
    let lang = i18n::ui_lang_from(cfg.ui.lang.as_deref())
        .map_err(|bad| format!("ui.lang 只支持 zh 或 en，当前是「{bad}」"))?;
    i18n::set_ui_lang(lang);
    Ok(())
}

/// 把 SIGPIPE 恢复成默认行为。
///
/// Rust 的 `std` 启动时会把 SIGPIPE 设为忽略，于是 `transgo lang | head` 这类完全正常的
/// 用法会在写 stdout 时抛出 `Broken pipe` panic。Unix 管道的语义就是「读端关了就安静退出」，
/// CLI 工具应当遵循，所以这里恢复成 `SIG_DFL`。
#[cfg(unix)]
fn restore_sigpipe() {
    const SIGPIPE: std::os::raw::c_int = 13;
    const SIG_DFL: usize = 0;
    extern "C" {
        fn signal(signum: std::os::raw::c_int, handler: usize) -> usize;
    }
    // SAFETY: SIG_DFL 是信号处理的合法取值，恢复默认处置不涉及内存安全
    unsafe {
        signal(SIGPIPE, SIG_DFL);
    }
}

#[cfg(not(unix))]
fn restore_sigpipe() {}

/// 让 `transgo "hello"` 与 `transgo translate "hello"` 等价。
///
/// clap 无法在一个命令里同时容纳「贪婪的位置参数」和「子命令」而不产生歧义，
/// 所以这里在解析前把默认子命令补上。判定是纯字符串比较，行为可预测。
fn normalize_argv() -> Vec<OsString> {
    const VERBS: &[&str] = &[
        "translate", "t", "tr", "engines", "config", "lang", "gui", "completions", "help",
    ];
    const GLOBALS: &[&str] = &["-h", "--help", "-V", "--version"];

    let mut argv: Vec<OsString> = std::env::args_os().collect();
    let first = argv.get(1).and_then(|s| s.to_str()).unwrap_or("");
    if argv.len() < 2 || (!VERBS.contains(&first) && !GLOBALS.contains(&first)) {
        argv.insert(1, OsString::from("translate"));
    }
    argv
}

const LONG_ABOUT: &str = r#"轻量快速的翻译工具，CLI 优先，Wayland / Hyprland 友好。

stdout 只有译文本身，元信息一律走 stderr，可以直接进管道。

示例：
  transgo "知识就是力量"          中英方向自动决策
  echo "good morning" | transgo   从 stdin 读
  transgo -t ja "你好"            指定目标语
  transgo -e deepl "hello"        指定翻译引擎
  transgo --json "hello"          完整结果，便于脚本处理
  transgo gui --clip              打开窗口翻译剪贴板内容

语种代码见 transgo lang，翻译引擎见 transgo engines。"#;

const LONG_ABOUT_EN: &str = r#"A lightweight, fast translation tool. CLI first, Wayland / Hyprland friendly.

stdout carries the translation only; metadata goes to stderr, so it pipes cleanly.

Examples:
  transgo "知识就是力量"          auto direction between Chinese and English
  echo "good morning" | transgo   read from stdin
  transgo -t ja "你好"            pick the target language
  transgo -e deepl "hello"        pick the engine
  transgo --json "hello"          full result, script friendly
  transgo gui --clip              open the GUI pre-filled from the clipboard

Language codes: transgo lang. Engines: transgo engines."#;

/// 中文帮助文案 → 英文。命中不了的原样保留（clap 自带脚手架本来就是英文）。
fn help_en(zh: &str) -> Option<&'static str> {
    if zh == LONG_ABOUT {
        return Some(LONG_ABOUT_EN);
    }
    Some(match zh {
        // 短 about（-h 用）与子命令
        "轻量快速的翻译工具，CLI 优先，Wayland / Hyprland 友好" => {
            "A lightweight, fast translation tool. CLI first, Wayland / Hyprland friendly"
        }
        "翻译文本（默认命令，可省略）" => "Translate text (default command; can be omitted)",
        "列出翻译引擎、免费额度和配置状态" => "List engines, free quotas and configuration status",
        "管理配置（API Key、默认引擎等）" => "Manage configuration (API keys, default engine, …)",
        "列出支持的语种" => "List supported languages",
        "打开图形界面翻译窗口" => "Open the GUI translation window",
        "生成 shell 补全脚本（zsh / bash / fish / elvish / powershell）" => {
            "Generate a shell completion script (zsh / bash / fish / elvish / powershell)"
        }
        // translate 参数
        "要翻译的文本，不传或传 - 则从 stdin 读取；以 - 开头的文本放到 -- 之后" => {
            "Text to translate; omit or pass - to read stdin. Put text starting with - after --"
        }
        "目标语种，如 zh / en / ja；auto = 中英互译，第三方语言看 default.lang" => {
            "Target language, e.g. zh / en / ja. auto = Chinese/English swap; third languages follow default.lang"
        }
        "源语种，不填就自动检测" => "Source language; auto-detected when omitted",
        "翻译引擎，不填则取第一个已配置的" => "Translation engine; defaults to the first configured one",
        "翻译指令，控制文风（仅 baidu-llm 生效），如「采用意译」" => {
            "Translation instruction controlling style (baidu-llm only), e.g. \"采用意译\""
        }
        "用 JSON 输出完整结果" => "Output the full result as JSON",
        "备选译文打到 stderr，stdout 保持干净好接管道" => {
            "Send alternative translations to stderr; stdout stays clean for pipes"
        }
        "在 stderr 附上引擎、语种和耗时" => "Add engine, languages and timing to stderr",
        // engines / lang 参数
        "对每个已配置的引擎实发一次请求，验证 Key 能用" => {
            "Send one real request per configured engine to verify the keys"
        }
        "附上申请地址和语种范围" => "Also show signup URLs and language ranges",
        "只列出某个引擎支持的语种" => "Show the language range of one engine",
        // gui / completions 参数
        "启动时读一次剪贴板，预填源文本并直接翻一次" => {
            "Read the clipboard once at start, pre-fill and translate immediately"
        }
        "目标 shell" => "Target shell",
        // config 子命令
        "显示配置文件路径" => "Print the config file path",
        "列出全部配置项" => "List every config key",
        "读取单项" => "Read one key",
        "写入单项，值传空串表示清除" => "Write one key (empty value clears it)",
        "清除单项" => "Clear one key",
        "生成带注释的模板配置" => "Generate a commented config template",
        "用 $EDITOR 打开配置文件" => "Open the config file in $EDITOR",
        "点分路径，例如 deepl.api_key 或 default.engine" => {
            "Dotted path, e.g. deepl.api_key or default.engine"
        }
        "要写入的值。数字和布尔直接写原样（如 15、true），传空串表示清除" => {
            "Value to write (numbers and booleans as-is, e.g. 15, true); empty string clears it"
        }
        _ => return None,
    })
}

/// 按界面语言把 clap 生成的帮助文案换掉。
/// 英文 README 的帮助块与英文输出逐字节对应，中文 README 同理。
fn localize_cmd(mut cmd: clap::Command) -> clap::Command {
    if i18n::ui_lang() != UiLang::En {
        return cmd;
    }
    if let Some(zh) = cmd.get_about().map(|x| x.to_string()) {
        if let Some(en) = help_en(&zh) {
            cmd = cmd.about(en);
        }
    }
    if let Some(zh) = cmd.get_long_about().map(|x| x.to_string()) {
        if let Some(en) = help_en(&zh) {
            cmd = cmd.long_about(en);
        }
    }
    let arg_ids: Vec<_> = cmd.get_arguments().map(|a| a.get_id().clone()).collect();
    for id in arg_ids {
        cmd = cmd.mut_arg(id, |mut a| {
            if let Some(zh) = a.get_help().map(|x| x.to_string()) {
                if let Some(en) = help_en(&zh) {
                    a = a.help(en);
                }
            }
            a
        });
    }
    let sub_names: Vec<_> = cmd.get_subcommands().map(|s| s.get_name().to_string()).collect();
    for name in sub_names {
        cmd = cmd.mut_subcommand(name, localize_cmd);
    }
    cmd
}

#[derive(Parser)]
#[command(
    name = "transgo",
    version,
    about = "轻量快速的翻译工具，CLI 优先，Wayland / Hyprland 友好",
    long_about = LONG_ABOUT,
    propagate_version = true,
    subcommand_required = true
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 翻译文本（默认命令，可省略）
    #[command(visible_alias = "t", alias = "tr")]
    Translate(TranslateArgs),

    /// 列出翻译引擎、免费额度和配置状态
    Engines(EnginesArgs),

    /// 管理配置（API Key、默认引擎等）
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },

    /// 列出支持的语种
    Lang(LangArgs),

    /// 打开图形界面翻译窗口
    Gui(GuiArgs),

    /// 生成 shell 补全脚本（zsh / bash / fish / elvish / powershell）
    Completions {
        /// 目标 shell
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Args)]
struct TranslateArgs {
    /// 要翻译的文本，不传或传 - 则从 stdin 读取；以 - 开头的文本放到 -- 之后
    #[arg(value_name = "TEXT")]
    text: Vec<String>,

    /// 目标语种，如 zh / en / ja；auto = 中英互译，第三方语言看 default.lang
    #[arg(short = 't', long = "to", value_name = "LANG")]
    to: Option<String>,

    /// 源语种，不填就自动检测
    #[arg(short = 'f', long = "from", value_name = "LANG")]
    from: Option<String>,

    /// 翻译引擎，不填则取第一个已配置的
    #[arg(short = 'e', long = "engine", value_name = "ID")]
    engine: Option<String>,

    /// 翻译指令，控制文风（仅 baidu-llm 生效），如「采用意译」
    #[arg(short = 'i', long = "instruction", value_name = "TEXT")]
    instruction: Option<String>,

    /// 用 JSON 输出完整结果
    #[arg(short = 'j', long = "json")]
    json: bool,

    /// 备选译文打到 stderr，stdout 保持干净好接管道
    #[arg(short = 'a', long = "alternatives")]
    alternatives: bool,

    /// 在 stderr 附上引擎、语种和耗时
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
}

#[derive(Args)]
struct EnginesArgs {
    /// 对每个已配置的引擎实发一次请求，验证 Key 能用
    #[arg(long)]
    test: bool,

    /// 附上申请地址和语种范围
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Args)]
struct LangArgs {
    /// 只列出某个引擎支持的语种
    #[arg(short, long, value_name = "ID")]
    engine: Option<String>,
}

#[derive(Args)]
struct GuiArgs {
    /// 启动时读一次剪贴板，预填源文本并直接翻一次
    #[arg(long)]
    clip: bool,
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// 显示配置文件路径
    Path,

    /// 列出全部配置项
    List,

    /// 读取单项
    Get {
        /// 点分路径，例如 deepl.api_key 或 default.engine
        key: String,
    },

    /// 写入单项，值传空串表示清除
    Set {
        /// 点分路径，例如 deepl.api_key 或 default.engine
        key: String,
        /// 要写入的值。数字和布尔直接写原样（如 15、true），传空串表示清除
        value: String,
    },

    /// 清除单项
    Unset {
        /// 点分路径，例如 deepl.api_key 或 default.engine
        key: String,
    },

    /// 生成带注释的模板配置
    Init {
        /// 即使文件已存在也覆盖
        #[arg(long)]
        force: bool,
    },

    /// 用 $EDITOR 打开配置文件
    Edit,
}

async fn run(cli: Cli) -> Result<(), Error> {
    match cli.cmd {
        Cmd::Translate(a) => cmd_translate(a).await,
        Cmd::Engines(a) => cmd_engines(a).await,
        Cmd::Config { action } => cmd_config(action),
        Cmd::Lang(a) => cmd_lang(a),
        Cmd::Gui(a) => transgo_gui::run(a.clip).map_err(|e| {
            Error::Other(match i18n::ui_lang() {
                UiLang::Zh => format!("启动图形界面失败: {e}"),
                UiLang::En => format!("Failed to start the GUI: {e}"),
            })
        }),
        Cmd::Completions { shell } => {
            let mut cmd = <Cli as clap::CommandFactory>::command();
            clap_complete::generate(shell, &mut cmd, "transgo", &mut std::io::stdout());
            Ok(())
        }
    }
}

// ---------------------------------------------------------------- translate

async fn cmd_translate(a: TranslateArgs) -> Result<(), Error> {
    let raw = if a.text.is_empty() || a.text == ["-".to_string()] {
        read_stdin()?
    } else {
        a.text.join(" ")
    };
    // 管道来的文本常带一个尾巴换行，去掉它免得污染译文
    let text = raw.trim_end_matches(['\n', '\r']);
    if text.trim().is_empty() {
        return Err(Error::Usage(
            s(
                "没有要翻译的文本。用法: transgo \"hello\"，或 echo hello | transgo",
                "No text to translate. Usage: transgo \"hello\", or echo hello | transgo",
            )
            .to_string(),
        ));
    }

    let cfg = Config::load()?;
    let from = parse_lang(a.from.as_deref().or(cfg.default.from.as_deref()))?;
    let to = parse_lang(a.to.as_deref().or(cfg.default.to.as_deref()))?;
    // auto 方向在这里定：中英互转，第三方语言翻向 default.lang
    let to = match to {
        Some(l) if l != Lang::Auto => Some(l),
        _ => {
            let third = transgo_core::detect::third_lang(cfg.default.lang.as_deref())
                .map_err(Error::Config)?;
            Some(transgo_core::detect::auto_target(text, third))
        }
    };
    let req = Request::new(text, from, to).with_instruction(a.instruction);

    let eng: Arc<dyn Engine> = match a.engine.as_deref() {
        Some(id) => engine::lookup(&cfg, id)?,
        None => engine::pick_default(&cfg, &req)?,
    };
    if !eng.configured() {
        return Err(Error::NotConfigured(eng.id()));
    }
    if !eng.supports(&req) {
        return Err(Error::unsupported_pair(
            eng.id(),
            req.from.unwrap_or(Lang::Auto),
            req.to,
        ));
    }

    let started = Instant::now();
    let out = eng.translate(&req).await?;
    let elapsed = started.elapsed();

    if a.json {
        let json = serde_json::to_string_pretty(&out).map_err(|e| Error::Other(e.to_string()))?;
        println!("{json}");
    } else {
        println!("{}", out.text);
        if a.alternatives && !out.alternatives.is_empty() {
            eprintln!("--- 备选译文 ---");
            for (i, alt) in out.alternatives.iter().enumerate() {
                eprintln!("{}. {alt}", i + 1);
            }
        }
    }

    if a.verbose {
        eprintln!(
            "[{}] {} → {} · {:.0} ms · {} 字符",
            out.engine,
            out.from.name_zh(),
            out.to.name_zh(),
            elapsed.as_secs_f64() * 1000.0,
            out.text.chars().count()
        );
    }
    Ok(())
}

fn read_stdin() -> Result<String, Error> {
    if std::io::stdin().is_terminal() {
        return Err(Error::Usage(
            s(
                "没有要翻译的文本。用法: transgo \"hello\"，或 echo hello | transgo",
                "No text to translate. Usage: transgo \"hello\", or echo hello | transgo",
            )
            .to_string(),
        ));
    }
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}

fn parse_lang(s: Option<&str>) -> Result<Option<Lang>, Error> {
    match s {
        None => Ok(None),
        Some(raw) => Lang::from_code(raw)
            .map(Some)
            .ok_or_else(|| Error::UnknownLang(raw.to_string())),
    }
}

// ------------------------------------------------------------------ engines

async fn cmd_engines(a: EnginesArgs) -> Result<(), Error> {
    let cfg = Config::load()?;
    let all = engine::build_all(&cfg);

    let w_id = all.iter().map(|e| dwidth(e.id())).max().unwrap_or(2).max(2);
    let w_name = all.iter().map(|e| dwidth(e.name())).max().unwrap_or(4).max(4);
    let w_state = 6usize;

    println!(
        "  {}  {}  {}  免费额度",
        pad("ID", w_id),
        pad("名称", w_name),
        pad("状态", w_state),
    );
    println!(
        "  {}  {}  {}  ────────",
        pad("──", w_id),
        pad("────", w_name),
        pad("────", w_state),
    );

    let probe = Request::new("hello", Some(Lang::En), Some(Lang::Zh));
    for e in &all {
        let configured = e.configured();
        let marker = if configured { "*" } else { " " };
        println!(
            "{marker} {}  {}  {}  {}",
            pad(e.id(), w_id),
            pad(e.name(), w_name),
            pad(if configured { "已配置" } else { "未配置" }, w_state),
            e.quota()
        );

        if a.verbose {
            println!("    申请: {}", e.signup());
            match e.languages() {
                Some(list) => println!("    语种: {} 个（受限）", list.len()),
                None => println!("    语种: 不限"),
            }
        }

        if a.test {
            let line = if !configured {
                "跳过（未配置）".to_string()
            } else if !e.supports(&probe) {
                "跳过（不支持 en → zh）".to_string()
            } else {
                let t0 = Instant::now();
                match e.translate(&probe).await {
                    Ok(t) => format!(
                        "OK   {:>5.0} ms   {}",
                        t0.elapsed().as_secs_f64() * 1000.0,
                        t.text.chars().take(16).collect::<String>()
                    ),
                    Err(err) => format!("失败  {err}"),
                }
            };
            println!("    测试: {line}");
        }
    }

    if a.test || a.verbose {
        println!();
    }
    println!("* = 已配置。默认引擎按上述顺序取第一个已配置项，可用 transgo config set default.engine <id> 固定。");
    Ok(())
}

// -------------------------------------------------------------------- config

fn cmd_config(action: ConfigCmd) -> Result<(), Error> {
    let path = config_path();
    match action {
        ConfigCmd::Path => {
            println!("{}", path.display());
        }
        ConfigCmd::List => {
            let cfg = Config::load()?;
            for line in cfg.keys() {
                println!("{line}");
            }
        }
        ConfigCmd::Get { key } => {
            let cfg = Config::load()?;
            match cfg.get(&key)? {
                Some(v) => println!("{v}"),
                None => println!("(未设置)"),
            }
        }
        ConfigCmd::Set { key, value } => {
            // 用 load_file：写回时不能把环境变量值一起持久化
            let mut cfg = Config::load_file()?;
            cfg.set(&key, &value)?;
            cfg.save(&path)?;
            eprintln!("已写入 {}", path.display());
        }
        ConfigCmd::Unset { key } => {
            let mut cfg = Config::load_file()?;
            cfg.set(&key, "")?;
            cfg.save(&path)?;
            eprintln!("已清除 {key}");
        }
        ConfigCmd::Init { force } => {
            if path.exists() && !force {
                return Err(Error::Usage(format!(
                    "{} 已存在。加 --force 覆盖，或用 transgo config set 修改单项",
                    path.display()
                )));
            }
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&path, template())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
            eprintln!("已生成 {}", path.display());
        }
        ConfigCmd::Edit => {
            if !path.exists() {
                Config::default().save(&path)?;
            }
            let editor = std::env::var("VISUAL")
                .or_else(|_| std::env::var("EDITOR"))
                .unwrap_or_else(|_| "nano".to_string());
            let status = std::process::Command::new(&editor)
                .arg(&path)
                .status()
                .map_err(|e| Error::Other(format!("启动编辑器 {editor} 失败: {e}")))?;
            if !status.success() {
                return Err(Error::Other(format!("编辑器 {editor} 退出异常")));
            }
        }
    }
    Ok(())
}

const TEMPLATE_ZH: &str = r#"# transgo 配置文件
# 改单项可用 transgo config set <key> <value>，比手改文件稳。所有键都能用环境变量
# TRANSGO_* 覆盖，例如 TRANSGO_DEEPL_API_KEY，环境变量优先于本文件。

[default]
# 默认引擎 id（见 transgo engines）。留空则按优先级取第一个已配置的。
engine = ""
# 目标语种。auto = 中英互转，第三方语言翻向下面的 lang
to = "auto"
# 源语种。auto = 自动检测
from = "auto"
# 第三方语言（日韩俄阿等非中非英）翻向哪种语言：zh 或 en，默认 en。中英之间永远互转
# lang = "en"
# 请求超时（秒）
timeout_secs = 15

# ---- 界面语言（CLI 与 GUI 共用）----
[ui]
# lang = "zh"    # zh 或 en，默认 zh

# ---- 图形界面 ----
[gui]
# 字体缩放系数（0.5 ~ 3.0）。高分屏嫌字大、外接屏嫌字小时调
# font_scale = 1.0

# ---- DeepL：免费 50 万字符/月。开通要境外发行的信用卡，国内的不认 ----
# https://www.deepl.com/pro-api 注册即可拿 Key，免费 Key 以 :fx 结尾
[deepl]
api_key = ""

# ---- 腾讯云机器翻译：500 万字符/月。2027-10-01 起全部接口不可用 ----
# https://cloud.tencent.com/document/product/551/40566
[tencent]
secret_id = ""
secret_key = ""
# region = "ap-guangzhou"

# ---- 火山引擎机器翻译：每月前 200 万字符免费，超出 49 元/百万字符自动扣费 ----
# https://console.volcengine.com/ai/region:ai+cn-north-1/translate
[volcano]
access_key_id = ""
secret_access_key = ""
# region = "cn-north-1"

# ---- 阿里云机器翻译（通用版）：100 万字符/月。超出自动转后付费且无法关闭 ----
# https://help.aliyun.com/zh/machine-translation/
[aliyun]
access_key_id = ""
access_key_secret = ""

# ---- 百度翻译：两个接口共用一套 APP ID + 密钥 ----
# baidu 走通用翻译 API（机器翻译）：标准版 5 万字符/月，实名认证后高级版 100 万字符/月
# baidu-llm 走大模型文本翻译 API：实名认证后一次性 100 万字符测试额度，不按月重置
# 超出后都是 49 元/百万字符，次日结算，余额不足即停，不会自动充值
# https://fanyi-api.baidu.com/
[baidu]
app_id = ""
secret = ""
# 可选：API Key 鉴权（控制台「API Key 管理」创建）。baidu-llm 填了就走 Bearer 免签名
# api_key = ""
# 可选：翻译指令（仅 baidu-llm），如「采用意译」，控制默认文风，单次可用 --instruction 覆盖
# reference = ""
# 可选：术语库干预（仅 baidu-llm）。术语表在控制台「我的术语库」维护，功能免费
# need_intervene = false

# ---- Azure AI Translator：200 万字符/月（需绑卡）----
# https://azure.microsoft.com/zh-cn/products/ai-services/ai-translator
[azure]
api_key = ""
# 多服务资源或区域资源必须填
# region = "eastasia"

# ---- Google Cloud Translation：50 万字符/月（国内要代理，且需绑卡开 billing）----
# https://cloud.google.com/translate/docs/setup
[google]
api_key = ""

# ---- 有道智云：无月度额度，一次性 50 元体验金。余额制，扣完即停 ----
# https://ai.youdao.com/
[youdao]
app_key = ""
app_secret = ""

# ---- 任意 OpenAI 兼容接口：OpenAI / DeepSeek / 硅基流动 / 本地 Ollama ----
[llm]
api_key = ""
base_url = "https://api.openai.com/v1"
model = "gpt-4o-mini"
# temperature = 0.2
# system_prompt = ""   # 覆盖默认翻译提示词，可用来固定术语表或语气

# ---- MyMemory：免注册，留邮箱可把额度提到 5 万字符/天 ----
[mymemory]
email = ""
"#;

/// 配置模板按界面语言出中/英文
fn template() -> &'static str {
    match i18n::ui_lang() {
        UiLang::Zh => TEMPLATE_ZH,
        UiLang::En => TEMPLATE_EN,
    }
}

const TEMPLATE_EN: &str = r#"# transgo config file
# Use transgo config set <key> <value> for single changes — safer than editing by hand.
# Every key can be overridden by a TRANSGO_* environment variable
# (e.g. TRANSGO_DEEPL_API_KEY), which takes precedence over this file.

[default]
# Default engine id (see transgo engines). Leave empty to take the first configured one by priority.
engine = ""
# Target language. auto = Chinese/English swap; third languages follow lang below
to = "auto"
# Source language. auto = auto-detect
from = "auto"
# Third languages (ja/ko/ru/ar and other non-Latin scripts) translate to zh or en, default en.
# Chinese and English always swap and are unaffected
# lang = "en"
# Request timeout in seconds
timeout_secs = 15

# ---- Interface language (shared by CLI and GUI) ----
[ui]
# lang = "zh"    # zh or en, default zh

# ---- GUI ----
[gui]
# Font scale (0.5 ~ 3.0). Turn it down on a laptop screen, up on an external monitor
# font_scale = 1.0

# ---- DeepL: 500 K characters/month free. Signup needs a credit card DeepL supports; cards issued in mainland China are not accepted ----
# https://www.deepl.com/pro-api — register to get a key; free keys end in :fx
[deepl]
api_key = ""

# ---- Tencent Cloud TMT: 5 M characters/month. Every API is retired 2027-10-01 ----
# https://cloud.tencent.com/document/product/551/40566
[tencent]
secret_id = ""
secret_key = ""
# region = "ap-guangzhou"

# ---- Volcano Engine MT: first 2 M characters/month free; beyond that 49 yuan per million, billed automatically ----
# https://console.volcengine.com/ai/region:ai+cn-north-1/translate
[volcano]
access_key_id = ""
secret_access_key = ""
# region = "cn-north-1"

# ---- Alibaba Cloud MT (General): 1 M characters/month. Switches to postpaid automatically beyond the quota and cannot be turned off ----
# https://help.aliyun.com/zh/machine-translation/
[aliyun]
access_key_id = ""
access_key_secret = ""

# ---- Baidu Translate: one APP ID + secret feeds both APIs ----
# baidu uses the General Translation API (machine translation): Standard 50 K characters/month, Advanced 1 M/month after identity verification
# baidu-llm uses the LLM Text Translation API: one-time 1 M-character test quota after identity verification, no monthly reset
# Both cost 49 yuan per million characters beyond the quota, settled the next day; service stops when the balance runs out, no auto top-up
# https://fanyi-api.baidu.com/
[baidu]
app_id = ""
secret = ""
# Optional: Bearer API key auth (created on the console's "API Key" page). baidu-llm then skips signing
# api_key = ""
# Optional: translation instruction (baidu-llm only), e.g. "采用意译", controlling the default style; --instruction overrides per call
# reference = ""
# Optional: glossary intervention (baidu-llm only). The glossary lives on the console's "My Glossary" page; the feature is free
# need_intervene = false

# ---- Azure AI Translator: 2 M characters/month (card required) ----
# https://azure.microsoft.com/zh-cn/products/ai-services/ai-translator
[azure]
api_key = ""
# Required for multi-service / regional resources
# region = "eastasia"

# ---- Google Cloud Translation: 500 K characters/month (proxy needed in mainland China, billing required) ----
# https://cloud.google.com/translate/docs/setup
[google]
api_key = ""

# ---- Youdao Cloud: no monthly quota, one-time 50-yuan trial credit. Balance-based, stops when used up ----
# https://ai.youdao.com/
[youdao]
app_key = ""
app_secret = ""

# ---- Any OpenAI-compatible API: OpenAI / DeepSeek / SiliconFlow / local Ollama ----
[llm]
api_key = ""
base_url = "https://api.openai.com/v1"
model = "gpt-4o-mini"
# temperature = 0.2
# system_prompt = ""   # overrides the default translation prompt — handy for glossaries or tone

# ---- MyMemory: registration-free; an email raises the quota to 50 K characters/day ----
[mymemory]
email = ""
"#;

// ---------------------------------------------------------------------- lang

fn cmd_lang(a: LangArgs) -> Result<(), Error> {
    let cfg = Config::load()?;
    let list: Vec<Lang> = match a.engine.as_deref() {
        Some(id) => {
            let e = engine::lookup(&cfg, id)?;
            match e.languages() {
                Some(l) => std::iter::once(Lang::Auto).chain(l.iter().copied()).collect(),
                None => std::iter::once(Lang::Auto).chain(Lang::ALL.iter().copied()).collect(),
            }
        }
        None => std::iter::once(Lang::Auto).chain(Lang::ALL.iter().copied()).collect(),
    };

    let w = list.iter().map(|l| dwidth(l.code())).max().unwrap_or(4).max(4);
    println!("{}  名称", pad("代码", w));
    println!("{}  ────", pad("────", w));
    for l in list {
        let note = if l == Lang::Auto {
            "（目标语为 auto 时按中英互译规则决策）"
        } else {
            ""
        };
        println!("{}  {}{note}", pad(l.code(), w), l.name_zh());
    }
    Ok(())
}

// ------------------------------------------------------- 按显示宽度对齐中文

fn dwidth(s: &str) -> usize {
    s.chars().map(|c| if is_wide(c) { 2 } else { 1 }).sum()
}

/// CJK 全角字符在终端占两列，按 `str::chars().count()` 对齐会错位
fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE10..=0xFE19
        | 0xFE30..=0xFE6F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x20000..=0x3FFFD)
}

fn pad(s: &str, w: usize) -> String {
    let mut out = s.to_string();
    let cur = dwidth(s);
    if cur < w {
        out.push_str(&" ".repeat(w - cur));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cjk_width_counts_double() {
        assert_eq!(dwidth("abc"), 3);
        assert_eq!(dwidth("中文"), 4);
        assert_eq!(pad("中", 4), "中  ");
    }

    #[test]
    fn flags_after_text_are_options_not_text() {
        let cli = Cli::try_parse_from(["transgo", "translate", "hello", "world", "-v"]).unwrap();
        let Cmd::Translate(a) = cli.cmd else { panic!("应解析成 translate") };
        assert_eq!(a.text, vec!["hello".to_string(), "world".to_string()]);
        assert!(a.verbose);
    }

    #[test]
    fn hyphen_text_goes_after_double_dash() {
        let cli = Cli::try_parse_from(["transgo", "translate", "--", "-v", "-5"]).unwrap();
        let Cmd::Translate(a) = cli.cmd else { panic!("应解析成 translate") };
        assert_eq!(a.text, vec!["-v".to_string(), "-5".to_string()]);
        assert!(!a.verbose);
    }

    #[test]
    fn bare_dash_stays_stdin_marker() {
        let cli = Cli::try_parse_from(["transgo", "translate", "-"]).unwrap();
        let Cmd::Translate(a) = cli.cmd else { panic!("应解析成 translate") };
        assert_eq!(a.text, vec!["-".to_string()]);
    }

    #[test]
    fn completions_takes_shell_arg() {
        let cli = Cli::try_parse_from(["transgo", "completions", "zsh"]).unwrap();
        assert!(matches!(cli.cmd, Cmd::Completions { .. }));
    }
}
