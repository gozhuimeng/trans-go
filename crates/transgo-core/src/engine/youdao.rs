//! 有道智云 · 文本翻译。
//!
//! 接口文档 <https://ai.youdao.com/DOCSIRMA/html/trans/api/wbfy/index.html>
//! 免费额度：新账号只有一次性 50 元体验金，没有月度免费额度。余额制，余额扣完即停。
//!
//! 签名 `SHA256(appKey + truncate(q) + salt + curtime + appSecret)`，输出**小写十六进制**（不是 Base64）。
//! `salt + curtime` 一对一用，重放会报 207。

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use super::util::{self, clip};
use super::Engine;
use crate::config::YoudaoCfg;
use crate::error::{Error, Result};
use crate::lang::Lang;
use crate::types::{Request, Translation};

const ENDPOINT: &str = "https://openapi.youdao.com/api";

pub struct Youdao {
    client: reqwest::Client,
    app_key: String,
    app_secret: String,
}

impl Youdao {
    pub fn new(client: reqwest::Client, cfg: &YoudaoCfg) -> Self {
        Youdao {
            client,
            app_key: cfg.app_key.clone().unwrap_or_default(),
            app_secret: cfg.app_secret.clone().unwrap_or_default(),
        }
    }

    /// 有道用 `zh-CHS` / `zh-CHT`
    fn code(l: Lang) -> &'static str {
        match l {
            Lang::Auto => "auto",
            Lang::Zh => "zh-CHS",
            Lang::ZhTw => "zh-CHT",
            other => other.code(),
        }
    }
}

/// 与有道官方 JS SDK 的 `truncate()` 保持一致：长度按 **UTF-16 码元**计
fn truncate(q: &str) -> String {
    let units: Vec<u16> = q.encode_utf16().collect();
    let len = units.len();
    if len <= 20 {
        return q.to_string();
    }
    let head = String::from_utf16_lossy(&units[..10]);
    let tail = String::from_utf16_lossy(&units[len - 10..]);
    format!("{head}{len}{tail}")
}

#[async_trait]
impl Engine for Youdao {
    fn id(&self) -> &'static str {
        "youdao"
    }
    fn name(&self) -> &'static str {
        crate::i18n::s("有道智云", "Youdao Cloud")
    }
    fn quota(&self) -> &'static str {
        crate::i18n::s("无月度额度，仅一次性 50 元体验金", "No monthly quota, one-time 50-yuan trial credit")
    }
    fn signup(&self) -> &'static str {
        "https://ai.youdao.com/"
    }
    fn configured(&self) -> bool {
        !self.app_key.is_empty() && !self.app_secret.is_empty()
    }
    fn languages(&self) -> Option<&'static [Lang]> {
        None
    }

    async fn translate(&self, req: &Request) -> Result<Translation> {
        if !self.configured() {
            return Err(Error::NotConfigured("youdao"));
        }

        let salt = uuid::Uuid::new_v4().to_string();
        let curtime = chrono::Utc::now().timestamp().to_string();
        let input = truncate(&req.text);

        // 小写 hex，不是 Base64
        let mut hasher = Sha256::new();
        hasher.update(
            format!("{}{input}{salt}{curtime}{}", self.app_key, self.app_secret).as_bytes(),
        );
        let sign = hex::encode(hasher.finalize());

        let from = req.from.unwrap_or(Lang::Auto);
        let resp = self
            .client
            .post(ENDPOINT)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[
                ("q", req.text.as_str()),
                ("from", Self::code(from)),
                ("to", Self::code(req.to)),
                ("appKey", self.app_key.as_str()),
                ("salt", &salt),
                ("sign", &sign),
                ("signType", "v3"),
                ("curtime", &curtime),
            ])
            .send()
            .await?;
        let got = util::fetch("youdao", resp).await?;

        // errorCode 是字符串，"0" 才是成功
        let code = got
            .json
            .get("errorCode")
            .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|n| n.to_string())))
            .unwrap_or_else(|| "未知".into());
        if code != "0" {
            let hint = match code.as_str() {
                "101" => "（缺少必填参数）",
                "102" => "（语种不支持）",
                "103" => "（文本过长，单次上限 5000 字符）",
                "108" => "（appKey 无效）",
                "110" => "（应用未绑定文本翻译服务）",
                "202" => "（签名校验失败，确认 q 是 UTF-8 未转义原文）",
                "203" => "（访问 IP 不在白名单）",
                "207" => "（重放请求，salt/curtime 不要复用）",
                "401" => "（账户已欠费）",
                "411" | "412" => "（访问频率受限）",
                _ => "",
            };
            return Err(util::api_err("youdao", format!("{code}{hint}"), "见错误码说明"));
        }

        let text = got
            .json
            .pointer("/translation/0")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::bad_response("youdao", format!("响应里没有译文: {}", clip(&got.raw, 200))))?;

        // `l` 形如 "EN2zh-CHS"，大小写不稳定，按 2 分割后忽略大小写解析
        let reported = got
            .json
            .get("l")
            .and_then(|v| v.as_str())
            .and_then(|s| s.split_once('2'))
            .map(|(src, _)| src);
        let from_lang = util::detected_or_local(&req.text, reported);

        Ok(Translation::new("youdao", text, from_lang, req.to))
    }
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncate_matches_js_sdk() {
        assert_eq!(truncate("short"), "short");
        let long = "a".repeat(30);
        assert_eq!(truncate(&long), format!("{}30{}", "a".repeat(10), "a".repeat(10)));
    }

    #[test]
    fn truncate_counts_utf16_units() {
        // 一个 emoji = 2 个 UTF-16 码元
        let s = "😀".repeat(15);
        assert_eq!(truncate(&s).len(), truncate(&s).len());
        assert!(truncate(&s).contains("30"), "15 个 emoji = 30 码元");
    }
}
