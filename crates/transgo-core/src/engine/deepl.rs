//! DeepL API —— 质量最好的官方接口，免费版每月 50 万字符。
//!
//! 接口文档 <https://developers.deepl.com/docs/api-reference/translate>
//! 免费额度 <https://support.deepl.com/hc/zh-cn/articles/360020685720>
//!
//! 免费 Key 以 `:fx` 结尾，走 `api-free.deepl.com`；Pro Key 走 `api.deepl.com`。
//! 也可以在配置里显式写 `plan` 或 `endpoint` 覆盖这个推断。

use async_trait::async_trait;

use super::util::{self, clip};
use super::Engine;
use crate::config::DeepLCfg;
use crate::error::{Error, Result};
use crate::lang::Lang;
use crate::types::{Request, Translation};

const ENDPOINT_FREE: &str = "https://api-free.deepl.com/v2/translate";
const ENDPOINT_PRO: &str = "https://api.deepl.com/v2/translate";

/// DeepL 免费版支持的语种
const SUPPORTED: &[Lang] = &[
    Lang::Zh,
    Lang::ZhTw,
    Lang::En,
    Lang::Ja,
    Lang::Ko,
    Lang::Fr,
    Lang::De,
    Lang::Es,
    Lang::Pt,
    Lang::It,
    Lang::Ru,
    Lang::Ar,
    Lang::Id,
    Lang::Tr,
    Lang::Nl,
    Lang::Pl,
    Lang::Sv,
    Lang::Da,
    Lang::Fi,
    Lang::El,
    Lang::Cs,
    Lang::Ro,
    Lang::Uk,
];

pub struct DeepL {
    client: reqwest::Client,
    api_key: String,
    endpoint: String,
}

impl DeepL {
    pub fn new(client: reqwest::Client, cfg: &DeepLCfg) -> Self {
        let api_key = cfg.api_key.clone().unwrap_or_default();
        let endpoint = cfg
            .endpoint
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                // `:fx` 是 DeepL 官方对免费 Key 的标记
                let is_free = cfg
                    .plan
                    .as_deref()
                    .map(|p| !p.eq_ignore_ascii_case("pro"))
                    .unwrap_or_else(|| api_key.trim_end().ends_with(":fx"));
                if is_free {
                    ENDPOINT_FREE.to_string()
                } else {
                    ENDPOINT_PRO.to_string()
                }
            });
        DeepL {
            client,
            api_key,
            endpoint,
        }
    }

    /// DeepL 用全大写代码，简中 `ZH`、繁中 `ZH-HANT`
    fn code(l: Lang) -> &'static str {
        match l {
            Lang::Auto => "",
            Lang::Zh => "ZH",
            Lang::ZhTw => "ZH-HANT",
            Lang::En => "EN",
            Lang::Ja => "JA",
            Lang::Ko => "KO",
            Lang::Fr => "FR",
            Lang::De => "DE",
            Lang::Es => "ES",
            Lang::Pt => "PT",
            Lang::It => "IT",
            Lang::Ru => "RU",
            Lang::Ar => "AR",
            Lang::Id => "ID",
            Lang::Tr => "TR",
            Lang::Nl => "NL",
            Lang::Pl => "PL",
            Lang::Sv => "SV",
            Lang::Da => "DA",
            Lang::Fi => "FI",
            Lang::El => "EL",
            Lang::Cs => "CS",
            Lang::Ro => "RO",
            Lang::Uk => "UK",
            other => other.code(),
        }
    }
}

#[async_trait]
impl Engine for DeepL {
    fn id(&self) -> &'static str {
        "deepl"
    }
    fn name(&self) -> &'static str {
        "DeepL"
    }
    fn quota(&self) -> &'static str {
        "50万字符/月"
    }
    fn signup(&self) -> &'static str {
        "https://www.deepl.com/pro-api"
    }
    fn configured(&self) -> bool {
        !self.api_key.is_empty()
    }
    fn languages(&self) -> Option<&'static [Lang]> {
        Some(SUPPORTED)
    }

    async fn translate(&self, req: &Request) -> Result<Translation> {
        if !self.configured() {
            return Err(Error::NotConfigured("deepl"));
        }

        // DeepL 要求保留换行的段落结构；这里整段送进去，让它自己断句
        let mut body = serde_json::json!({
            "text": [req.text.as_str()],
            "target_lang": Self::code(req.to),
            "preserve_formatting": true,
        });
        if let Some(from) = req.from.filter(|l| *l != Lang::Auto) {
            body["source_lang"] = serde_json::Value::String(Self::code(from).to_string());
        }

        let resp = self
            .client
            .post(&self.endpoint)
            .header("Authorization", format!("DeepL-Auth-Key {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        let got = util::fetch("deepl", resp).await?;

        if !got.status.is_success() {
            let msg = util::message_of(&got.json)
                .unwrap_or_else(|| clip(&got.raw, 200));
            let hint = match got.status.as_u16() {
                403 => "（Key 无效，或免费 Key 被发到了 Pro 接口）",
                456 => "（免费额度已用完，下月重置）",
                429 => "（请求过于频繁）",
                _ => "",
            };
            return Err(util::api_err(
                "deepl",
                format!("{}{hint}", got.status.as_u16()),
                msg,
            ));
        }

        let first = got
            .json
            .pointer("/translations/0")
            .ok_or_else(|| Error::bad_response("deepl", format!("响应里没有译文: {}", clip(&got.raw, 200))))?;

        let text = first
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::bad_response("deepl", "translations[0].text 缺失"))?;

        let reported = first
            .get("detected_source_language")
            .and_then(|v| v.as_str());
        let from_lang = util::detected_or_local(&req.text, reported);

        Ok(Translation::new("deepl", text, from_lang, req.to))
    }
}
