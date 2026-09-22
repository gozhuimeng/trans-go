//! 统一错误类型。所有面向用户的文案都是中文，CLI 直接打印即可。

use crate::Lang;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("引擎「{0}」未配置，用 transgo config 填入 API Key，申请地址看 transgo engines")]
    NotConfigured(&'static str),

    #[error("未知引擎「{0}」，可用引擎见 transgo engines")]
    UnknownEngine(String),

    #[error("未知语种代码「{0}」，可用语种见 transgo lang")]
    UnknownLang(String),

    #[error("引擎「{engine}」不支持 {from} → {to} 这个方向")]
    UnsupportedPair {
        engine: &'static str,
        from: String,
        to: String,
    },

    #[error("网络请求失败: {0}")]
    Http(#[from] reqwest::Error),

    #[error("{engine} 返回错误 [{code}]: {message}")]
    Api {
        engine: &'static str,
        code: String,
        message: String,
    },

    #[error("{engine} 返回了无法解析的响应: {message}")]
    BadResponse {
        engine: &'static str,
        message: String,
    },

    #[error("配置错误: {0}")]
    Config(String),

    #[error("{0}")]
    Usage(String),

    #[error("IO 错误: {0}")]
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
        Error::UnsupportedPair {
            engine,
            from: from.name_zh().to_string(),
            to: to.name_zh().to_string(),
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
