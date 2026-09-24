//! 统一错误类型。面向用户的文案按 [`crate::i18n`] 的界面语言出中/英文，CLI 直接打印即可。

use crate::i18n::{ui_lang, UiLang};
use crate::Lang;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}", match ui_lang() {
        UiLang::Zh => format!("引擎「{}」未配置，用 transgo config 填入 API Key，申请地址看 transgo engines", .0),
        UiLang::En => format!("Engine '{}' is not configured. Add an API key with transgo config; see transgo engines for signup URLs", .0),
    })]
    NotConfigured(&'static str),

    #[error("{}", match ui_lang() {
        UiLang::Zh => format!("未知引擎「{}」，可用引擎见 transgo engines", .0),
        UiLang::En => format!("Unknown engine '{}'. See transgo engines for available ids", .0),
    })]
    UnknownEngine(String),

    #[error("{}", match ui_lang() {
        UiLang::Zh => format!("未知语种代码「{}」，可用语种见 transgo lang", .0),
        UiLang::En => format!("Unknown language code '{}'. See transgo lang for available codes", .0),
    })]
    UnknownLang(String),

    #[error("{}", match ui_lang() {
        UiLang::Zh => format!("引擎「{}」不支持 {} → {} 这个方向", .engine, .from, .to),
        UiLang::En => format!("Engine '{}' does not support {} → {}", .engine, .from, .to),
    })]
    UnsupportedPair {
        engine: &'static str,
        from: String,
        to: String,
    },

    #[error("{}", match ui_lang() {
        UiLang::Zh => format!("网络请求失败: {}", .0),
        UiLang::En => format!("Network request failed: {}", .0),
    })]
    Http(#[from] reqwest::Error),

    #[error("{}", match ui_lang() {
        UiLang::Zh => format!("{} 返回错误 [{}]: {}", .engine, .code, .message),
        UiLang::En => format!("{} returned error [{}]: {}", .engine, .code, .message),
    })]
    Api {
        engine: &'static str,
        code: String,
        message: String,
    },

    #[error("{}", match ui_lang() {
        UiLang::Zh => format!("{} 返回了无法解析的响应: {}", .engine, .message),
        UiLang::En => format!("{} returned an unparseable response: {}", .engine, .message),
    })]
    BadResponse {
        engine: &'static str,
        message: String,
    },

    #[error("{}", match ui_lang() {
        UiLang::Zh => format!("配置错误: {}", .0),
        UiLang::En => format!("Configuration error: {}", .0),
    })]
    Config(String),

    #[error("{0}")]
    Usage(String),

    #[error("{}", match ui_lang() {
        UiLang::Zh => format!("IO 错误: {}", .0),
        UiLang::En => format!("IO error: {}", .0),
    })]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn api(engine: &'static str, code: impl Into<String>, message: impl Into<String>) -> Self {
        Error::Api {
            engine,
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn bad_response(engine: &'static str, message: impl Into<String>) -> Self {
        Error::BadResponse {
            engine,
            message: message.into(),
        }
    }

    pub fn unsupported_pair(engine: &'static str, from: Lang, to: Lang) -> Self {
        let (from_n, to_n) = match ui_lang() {
            UiLang::Zh => (from.name_zh(), to.name_zh()),
            UiLang::En => (from.name_en(), to.name_en()),
        };
        Error::UnsupportedPair {
            engine,
            from: from_n.to_string(),
            to: to_n.to_string(),
        }
    }

    /// 进程退出码：配置类问题用 2，远端错误用 3，其余用 1
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::NotConfigured(_)
            | Error::UnknownEngine(_)
            | Error::UnknownLang(_)
            | Error::Config(_)
            | Error::Usage(_) => 2,
            Error::Api { .. } | Error::BadResponse { .. } | Error::UnsupportedPair { .. } => 3,
            _ => 1,
        }
    }
}
