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

use clap::{Args, Parser, Subcommand};

use transgo_core::config::config_path;
use transgo_core::engine::{self, Engine};
use transgo_core::{Config, Error, Lang, Request};

#[tokio::main]
async fn main() -> ExitCode {
    restore_sigpipe();
    let cli = Cli::parse_from(normalize_argv());
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("transgo: {e}");
            ExitCode::from(e.exit_code().clamp(0, 255) as u8)
        }
    }
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
    const VERBS: &[&str] = &["translate", "t", "tr", "engines", "config", "lang", "help"];
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

语种代码见 transgo lang，翻译引擎见 transgo engines。"#;

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
}#[derive(Args)]
struct TranslateArgs {
    /// 要翻译的文本，不传或传 - 则从 stdin 读取
    #[arg(value_name = "TEXT", trailing_var_arg = true, allow_hyphen_values = true)]
    text: Vec<String>,

    /// 目标语种，如 zh / en / ja；auto = 中文译英文，其余译中文
    #[arg(short = 't', long = "to", value_name = "LANG")]
    to: Option<String>,

    /// 源语种，不填就自动检测
    #[arg(short = 'f', long = "from", value_name = "LANG")]
    from: Option<String>,

    /// 翻译引擎，不填则取第一个已配置的
    #[arg(short = 'e', long = "engine", value_name = "ID")]
    engine: Option<String>,

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
            "没有要翻译的文本。用法: transgo \"hello\"，或 echo hello | transgo".into(),
        ));
    }

    let cfg = Config::load()?;
    let from = parse_lang(a.from.as_deref().or(cfg.default.from.as_deref()))?;
    let to = parse_lang(a.to.as_deref().or(cfg.default.to.as_deref()))?;
    let req = Request::new(text, from, to);

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
            "没有要翻译的文本。用法: transgo \"hello\"，或 echo hello | transgo".into(),
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
            std::fs::write(&path, TEMPLATE)?;
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

const TEMPLATE: &str = r#"# transgo 配置文件
# 改单项可用 transgo config set <key> <value>，比手改文件稳。所有键都能用环境变量
# TRANSGO_* 覆盖，例如 TRANSGO_DEEPL_API_KEY，环境变量优先于本文件。

[default]
# 默认引擎 id（见 transgo engines）。留空则按优先级取第一个已配置的。
engine = ""
# 目标语种。auto = 中英互译（中文译英文，其余一律译中文）
to = "auto"
# 源语种。auto = 自动检测
from = "auto"
# 请求超时（秒）
timeout_secs = 15

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
}
