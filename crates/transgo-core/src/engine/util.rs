//! 各引擎共用的小工具。

use crate::error::{Error, Result};
use crate::lang::Lang;

/// 已抓取到的响应：状态码 + 尽力解析出的 JSON + 原始正文
pub struct Fetched {
    pub status: reqwest::StatusCode,
    pub json: serde_json::Value,
    pub raw: String,
}

/// 读响应体。**不**在这里判定成败 —— 有些引擎 HTTP 200 里也带错误，交给各引擎自己看。
pub async fn fetch(engine: &'static str, resp: reqwest::Response) -> Result<Fetched> {
    let status = resp.status();
    let raw = resp.text().await?;
    let json = serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null);
    if json.is_null() && !raw.trim().is_empty() && status.is_success() {
        return Err(Error::bad_response(engine, format!("正文不是 JSON: {}", clip(&raw, 200))));
    }
    Ok(Fetched { status, json, raw })
}

/// 引擎报错时的统一出口
pub fn api_err(engine: &'static str, code: impl Into<String>, msg: impl Into<String>) -> Error {
    let msg = msg.into();
    if msg.is_empty() {
        Error::api(engine, code, "无附加信息")
    } else {
        Error::api(engine, code, msg)
    }
}

/// 从各家五花八门的错误体里抠出人话
pub fn message_of(json: &serde_json::Value) -> Option<String> {
    for path in [
        "/error/message",
        "/error/Message",
        "/message",
        "/Message",
        "/error_msg",
        "/msg",
        "/error_description",
    ] {
        if let Some(serde_json::Value::String(s)) = json.pointer(path) {
            if !s.is_empty() {
                return Some(s.clone());
            }
        }
    }
    // 套在 Response / ResponseMetadata 里的
    for path in [
        "/Response/Error/Message",
        "/ResponseMetadata/Error/Message",
        "/error/text",
    ] {
        if let Some(serde_json::Value::String(s)) = json.pointer(path) {
            if !s.is_empty() {
                return Some(s.clone());
            }
        }
    }
    None
}

/// 引擎报告的源语种；没报告或解析不了就退回本地字符类别探测
pub fn detected_or_local(text: &str, reported: Option<&str>) -> Lang {
    reported
        .and_then(Lang::from_code)
        .filter(|l| *l != Lang::Auto)
        .unwrap_or_else(|| crate::detect::detect(text))
}

/// 长文本截断，用于错误提示
pub fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

/// MyMemory 之类的接口会把译文做 HTML 实体转义，这里还原最常见的几个
pub fn unescape_html(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&") // 必须放最后
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescape_handles_ampersand_last() {
        assert_eq!(unescape_html("a &amp; b &lt;c&gt;"), "a & b <c>");
        assert_eq!(unescape_html("plain"), "plain");
    }

    #[test]
    fn clip_truncates_by_chars() {
        assert_eq!(clip("abcdef", 3), "abc…");
        assert_eq!(clip("ab", 3), "ab");
    }

    #[test]
    fn detected_falls_back_to_local() {
        assert_eq!(detected_or_local("你好", None), Lang::Zh);
        assert_eq!(detected_or_local("hi", Some("EN")), Lang::En);
        assert_eq!(detected_or_local("hi", Some("garbage")), Lang::En);
    }
}
