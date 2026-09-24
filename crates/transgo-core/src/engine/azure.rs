//! Microsoft Azure AI Translator 文本翻译 v3。
//!
//! 接口文档 <https://learn.microsoft.com/zh-cn/azure/ai-services/translator/reference/v3-translate>
//! 免费额度：F0 层每月 200 万字符。
//!
//! 注意 Azure 不在翻译响应里回传检测到的源语种，这里退化为本地字符类别探测。

use async_trait::async_trait;

use super::util::{self, clip};
use super::Engine;
use crate::config::AzureCfg;
use crate::error::{Error, Result};
use crate::lang::Lang;
use crate::types::{Request, Translation};

const DEFAULT_ENDPOINT: &str = "https://api.cognitive.microsofttranslator.com";

/// Azure 支持的语种（F0 通用模型）
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
    Lang::Th,
    Lang::Vi,
    Lang::Id,
    Lang::Ms,
    Lang::Tr,
    Lang::Hi,
    Lang::Nl,
    Lang::Pl,
    Lang::Sv,
    Lang::Da,
    Lang::Fi,
    Lang::El,
    Lang::Cs,
    Lang::Ro,
    Lang::Uk,
    Lang::Fa,
    Lang::He,
];

pub struct Azure {
    client: reqwest::Client,
    api_key: String,
    region: Option<String>,
    endpoint: String,
}

impl Azure {
    pub fn new(client: reqwest::Client, cfg: &AzureCfg) -> Self {
        Azure {
            client,
            api_key: cfg.api_key.clone().unwrap_or_default(),
            region: cfg.region.clone().filter(|s| !s.is_empty()),
            endpoint: cfg
                .endpoint
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string()),
        }
    }

    /// Azure 用 `zh-Hans`/`zh-Hant`
    fn code(l: Lang) -> &'static str {
        match l {
            Lang::Auto => "",
            Lang::Zh => "zh-Hans",
            Lang::ZhTw => "zh-Hant",
            other => other.code(),
        }
    }
}

#[async_trait]
impl Engine for Azure {
    fn id(&self) -> &'static str {
        "azure"
    }
    fn name(&self) -> &'static str {
        "Azure AI Translator"
    }
    fn quota(&self) -> &'static str {
        crate::i18n::s("200万字符/月", "2 M characters/month")
    }
    fn signup(&self) -> &'static str {
        "https://learn.microsoft.com/zh-cn/azure/ai-services/translator/create-translator-resource"
    }
    fn configured(&self) -> bool {
        !self.api_key.is_empty()
    }
    fn languages(&self) -> Option<&'static [Lang]> {
        Some(SUPPORTED)
    }

    async fn translate(&self, req: &Request) -> Result<Translation> {
        if !self.configured() {
            return Err(Error::NotConfigured("azure"));
        }

        let mut query = vec![
            ("api-version", "3.0"),
            ("to", Self::code(req.to)),
            ("textType", "plain"),
        ];
        if let Some(from) = req.from.filter(|l| *l != Lang::Auto) {
            query.push(("from", Self::code(from)));
        }

        let url = format!("{}/translate", self.endpoint.trim_end_matches('/'));
        let mut call = self
            .client
            .post(&url)
            .query(&query)
            .header("Ocp-Apim-Subscription-Key", self.api_key.as_str())
            .header("Content-Type", "application/json")
            .header("X-ClientTraceId", uuid::Uuid::new_v4().to_string())
            .json(&serde_json::json!([{ "Text": req.text.as_str() }]));

        // 多服务资源 / 区域资源必须带这个头，否则 401001
        if let Some(region) = &self.region {
            call = call.header("Ocp-Apim-Subscription-Region", region.as_str());
        }

        let got = util::fetch("azure", call.send().await?).await?;

        if !got.status.is_success() {
            let code = got
                .json
                .pointer("/error/code")
                .and_then(|v| v.as_i64())
                .map(|n| n.to_string())
                .unwrap_or_else(|| got.status.as_u16().to_string());
            let msg = util::message_of(&got.json).unwrap_or_else(|| clip(&got.raw, 200));
            let hint = match code.as_str() {
                "401000" => "（API Key 无效）",
                "401001" => "（缺少 Ocp-Apim-Subscription-Region，请在配置里填 azure.region）",
                "403000" => "（无权限或配额耗尽）",
                _ => "",
            };
            return Err(util::api_err("azure", format!("{code}{hint}"), msg));
        }

        let text = got
            .json
            .pointer("/0/translations/0/text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::bad_response("azure", format!("响应里没有译文: {}", clip(&got.raw, 200))))?;

        // Azure 不回传检测语种，只能本地猜
        let from_lang = util::detected_or_local(&req.text, None);

        Ok(Translation::new("azure", text, from_lang, req.to))
    }
}
