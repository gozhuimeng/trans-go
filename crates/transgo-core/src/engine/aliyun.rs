//! 阿里云机器翻译 · 通用版（`alimt` / `2018-10-12` / `TranslateGeneral`）。
//!
//! 接口文档 <https://help.aliyun.com/zh/machine-translation/developer-reference/api-alimt-2018-10-12-translategeneral>
//! 免费额度：通用版 100 万字符/月，每月 1 日重置。
//!
//! 注意：`Translate` 是**专业版**（60 元/百万字符），通用版才是 `TranslateGeneral`。
//! 鉴权是阿里云 POP-RPC 的 HMAC-SHA1，对查询串做两次百分号编码。

use async_trait::async_trait;
use hmac::{Hmac, Mac};
use sha1::Sha1;

use super::util::{self, clip};
use super::Engine;
use crate::config::AliyunCfg;
use crate::error::{Error, Result};
use crate::lang::Lang;
use crate::types::{Request, Translation};

const ENDPOINT: &str = "https://mt.aliyuncs.com";

type HmacSha1 = Hmac<Sha1>;

pub struct Aliyun {
    client: reqwest::Client,
    access_key_id: String,
    access_key_secret: String,
}

impl Aliyun {
    pub fn new(client: reqwest::Client, cfg: &AliyunCfg) -> Self {
        Aliyun {
            client,
            access_key_id: cfg.access_key_id.clone().unwrap_or_default(),
            access_key_secret: cfg.access_key_secret.clone().unwrap_or_default(),
        }
    }

    /// alimt 用 ISO 风格代码，繁中是 `zh-tw`
    fn code(l: Lang) -> &'static str {
        match l {
            Lang::Auto => "auto",
            Lang::ZhTw => "zh-tw",
            other => other.code(),
        }
    }
}

/// 阿里云 RPC 规范的百分号编码：RFC 3986 保留字符一律转义，`~` 保持原样
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[async_trait]
impl Engine for Aliyun {
    fn id(&self) -> &'static str {
        "aliyun"
    }
    fn name(&self) -> &'static str {
        crate::i18n::s("阿里云机器翻译", "Alibaba Cloud MT")
    }
    fn quota(&self) -> &'static str {
        crate::i18n::s("通用版 100万字符/月", "General edition, 1 M characters/month")
    }
    fn signup(&self) -> &'static str {
        "https://help.aliyun.com/zh/machine-translation/product-overview/activate-service"
    }
    fn configured(&self) -> bool {
        !self.access_key_id.is_empty() && !self.access_key_secret.is_empty()
    }
    fn languages(&self) -> Option<&'static [Lang]> {
        // alimt 有 215 个语种且都是 ISO 风格代码，这里不做白名单限制
        None
    }

    async fn translate(&self, req: &Request) -> Result<Translation> {
        if !self.configured() {
            return Err(Error::NotConfigured("aliyun"));
        }

        let now = chrono::Utc::now();
        let mut params: Vec<(&str, String)> = vec![
            ("Action", "TranslateGeneral".into()),
            ("Format", "JSON".into()),
            ("FormatType", "text".into()),
            ("Scene", "general".into()),
            ("SourceLanguage", Self::code(req.from.unwrap_or(Lang::Auto)).into()),
            ("SourceText", req.text.clone()),
            ("TargetLanguage", Self::code(req.to).into()),
            ("AccessKeyId", self.access_key_id.clone()),
            ("SignatureMethod", "HMAC-SHA1".into()),
            ("SignatureNonce", uuid::Uuid::new_v4().to_string()),
            ("SignatureVersion", "1.0".into()),
            ("Timestamp", now.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
            ("Version", "2018-10-12".into()),
        ];
        params.sort_by(|a, b| a.0.cmp(b.0));

        let canonical_query = params
            .iter()
            .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        // 注意 StringToSign 里对 canonical_query 再编码一次
        let string_to_sign = format!("GET&%2F&{}", percent_encode(&canonical_query));

        let mut signer = HmacSha1::new_from_slice(format!("{}&", self.access_key_secret).as_bytes())
            .expect("HMAC-SHA1 接受任意长度密钥");
        signer.update(string_to_sign.as_bytes());
        let signature = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            signer.finalize().into_bytes(),
        );

        let url = format!("{ENDPOINT}/?{canonical_query}&Signature={}", percent_encode(&signature));
        let got = util::fetch("aliyun", self.client.get(&url).send().await?).await?;

        // 业务错误与网关错误的 Code 可能是整数也可能是字符串
        let code = got.json.get("Code").cloned();
        let ok = matches!(&code, Some(serde_json::Value::Number(n)) if n.as_i64() == Some(200));
        if !got.status.is_success() || !ok {
            let code_str = match code {
                Some(serde_json::Value::Number(n)) => n.to_string(),
                Some(serde_json::Value::String(s)) => s,
                _ => got.status.as_u16().to_string(),
            };
            let msg = util::message_of(&got.json).unwrap_or_else(|| clip(&got.raw, 200));
            let hint = match code_str.as_str() {
                "10005" => "（语种方向不支持）",
                "10008" => "（文本过长，单次上限 5000 字符）",
                "10010" | "10013" => "（服务未开通或欠费）",
                "InvalidAccessKeyId.NotFound" => "（AccessKey ID 无效）",
                "SignatureDoesNotMatch" => "（AccessKey Secret 无效）",
                "Forbidden.RAM" => "（RAM 子账号无 alimt 权限）",
                _ => "",
            };
            return Err(util::api_err("aliyun", format!("{code_str}{hint}"), msg));
        }

        let text = got
            .json
            .pointer("/Data/Translated")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::bad_response("aliyun", format!("响应里没有译文: {}", clip(&got.raw, 200))))?;

        // 只有请求带 SourceLanguage=auto 时才回填 DetectedLanguage
        let reported = got
            .json
            .pointer("/Data/DetectedLanguage")
            .and_then(|v| v.as_str());
        let from_lang = util::detected_or_local(&req.text, reported);

        Ok(Translation::new("aliyun", text, from_lang, req.to))
    }
}
