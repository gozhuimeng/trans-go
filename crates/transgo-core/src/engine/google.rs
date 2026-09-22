//! Google Cloud Translation v2（Basic）。
//!
//! 接口文档 <https://cloud.google.com/translate/docs/reference/translate-text-v2>
//! 免费额度：每月前 50 万字符免费，按月重置、不过期。
//!
//! 用的是**官方** `translation.googleapis.com` + API Key，不是网页版逆向接口。

use async_trait::async_trait;

use super::util::{self, clip};
use super::Engine;
use crate::config::KeyCfg;
use crate::error::{Error, Result};
use crate::lang::Lang;
use crate::types::{Request, Translation};

const ENDPOINT: &str = "https://translation.googleapis.com/language/translate/v2";

pub struct Google {
    client: reqwest::Client,
    api_key: String,
}

impl Google {
    pub fn new(client: reqwest::Client, cfg: &KeyCfg) -> Self {
        Google {
            client,
            api_key: cfg.api_key.clone().unwrap_or_default(),
        }
    }

    /// Google 用 `zh-CN`/`zh-TW`，希伯来语是遗留代码 `iw`
    fn code(l: Lang) -> &'static str {
        match l {
            Lang::Auto => "",
            Lang::Zh => "zh-CN",
            Lang::ZhTw => "zh-TW",
            Lang::He => "iw",
            other => other.code(),
        }
    }
}

#[async_trait]
impl Engine for Google {
    fn id(&self) -> &'static str {
        "google"
    }
    fn name(&self) -> &'static str {
        "Google Cloud Translation"
    }
    fn quota(&self) -> &'static str {
        "50万字符/月"
    }
    fn signup(&self) -> &'static str {
        "https://cloud.google.com/translate/docs/setup"
    }
    fn configured(&self) -> bool {
        !self.api_key.is_empty()
    }
    fn languages(&self) -> Option<&'static [Lang]> {
        None
    }

    async fn translate(&self, req: &Request) -> Result<Translation> {
        if !self.configured() {
            return Err(Error::NotConfigured("google"));
        }

        let mut body = serde_json::json!({
            "q": req.text.as_str(),
            "target": Self::code(req.to),
            "format": "text",
        });
        if let Some(from) = req.from.filter(|l| *l != Lang::Auto) {
            body["source"] = serde_json::Value::String(Self::code(from).to_string());
        }

        let resp = self
            .client
            .post(ENDPOINT)
            .query(&[("key", self.api_key.as_str())])
            .json(&body)
            .send()
            .await?;
        let got = util::fetch("google", resp).await?;

        if !got.status.is_success() {
            let code = got
                .json
                .pointer("/error/code")
                .and_then(|v| v.as_i64())
                .map(|n| n.to_string())
                .unwrap_or_else(|| got.status.as_u16().to_string());
            let msg = util::message_of(&got.json).unwrap_or_else(|| clip(&got.raw, 200));
            let hint = match got.status.as_u16() {
                400 => "（API Key 无效或请求格式有问题）",
                403 => "（API Key 无权限，需在 GCP 里启用 Cloud Translation API）",
                429 => "（配额已用尽）",
                _ => "",
            };
            return Err(util::api_err("google", format!("{code}{hint}"), msg));
        }

        let first = got
            .json
            .pointer("/data/translations/0")
            .ok_or_else(|| Error::bad_response("google", format!("响应里没有译文: {}", clip(&got.raw, 200))))?;

        let text = first
            .get("translatedText")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::bad_response("google", "data.translations[0].translatedText 缺失"))?;

        let reported = first
            .get("detectedSourceLanguage")
            .and_then(|v| v.as_str());
        let from_lang = util::detected_or_local(&req.text, reported);

        Ok(Translation::new("google", text, from_lang, req.to))
    }
}
