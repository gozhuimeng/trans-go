//! `transgo-core` —— 翻译引擎抽象层，CLI 与 GUI 共用。
//!
//! 提供四块能力：
//! - [`lang`]：统一语种枚举 + 各家引擎语种代码映射
//! - [`detect`]：零依赖的轻量语种探测（服务于「中英互译」的目标语自动决策）
//! - [`config`]：`~/.config/transgo/config.toml` 的读写
//! - [`engine`]：[`engine::Engine`] trait 与 10 个官方翻译接口的实现

pub mod config;
pub mod detect;
pub mod engine;
pub mod error;
pub mod lang;
pub mod types;

pub use config::Config;
pub use error::{Error, Result};
pub use lang::Lang;
pub use types::{Request, Translation};

use std::time::Duration;

/// 默认请求超时
pub const DEFAULT_TIMEOUT_SECS: u64 = 15;

/// 各引擎共用的 HTTP client 构造
pub(crate) fn http_client(timeout: Duration) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("transgo/", env!("CARGO_PKG_VERSION")))
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(8))
        .pool_max_idle_per_host(2)
        .build()
        .map_err(Error::from)
}

/// 当前进程使用的超时
pub(crate) fn timeout_from(cfg: &Config) -> Duration {
    Duration::from_secs(
        cfg.default
            .timeout_secs
            .unwrap_or(DEFAULT_TIMEOUT_SECS),
    )
}
