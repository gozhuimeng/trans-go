//! 火山引擎机器翻译 · `TranslateText`。
//!
//! 接口文档 <https://docs.volcengine.com/docs/MachineTranslation/TextTranslationAPI?lang=zh>
//! 免费额度：**每月 200 万字符**。
//!
//! 鉴权是火山引擎 V4 签名（HMAC-SHA256），不是有些第三方客户端用的 `X-Top-*` 那套。
//! 源语种要**省略字段**来表示自动检测，不能传 `"auto"`。
//! 语种代码是 BCP-47 风格，繁中是 `zh-Hant`（注意 `tw` 在这里是契维语，不是繁中）。

use std::sync::atomic::{AtomicU8, Ordering};

use async_trait::async_trait;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use super::util::{self, clip};
use super::Engine;
use crate::config::VolcanoCfg;
use crate::error::{Error, Result};
use crate::lang::Lang;
use crate::types::{Request, Translation};

const HOST: &str = "translate.volcengineapi.com";
const SERVICE: &str = "translate";
const VERSION: &str = "2020-06-01";
const ACTION: &str = "TranslateText";
const DEFAULT_REGION: &str = "cn-north-1";

type HmacSha256 = Hmac<Sha256>;

/// `StringToSign` 第二行用哪种时间形式。
///
/// 官方文档（函数服务 / 视频直播 / 搜索服务三处）都写明 `RequestDate` 是 `YYYYMMDD'T'HHMMSS'Z'`，
/// 但 `volc-openapi-demos` 的示例代码里变量名是 `date`，容易被读成短日期。
/// 因为无法离线实测，这里首次遇到签名错误时会自动换一种形式重试一次，并把结论记住。
const STYLE_UNKNOWN: u8 = 0;
const STYLE_FULL_DATETIME: u8 = 1;
const STYLE_SHORT_DATE: u8 = 2;

pub struct Volcano {
    client: reqwest::Client,
    access_key_id: String,
    secret_access_key: String,
    region: String,
    date_style: AtomicU8,
}

impl Volcano {
    pub fn new(client: reqwest::Client, cfg: &VolcanoCfg) -> Self {
        Volcano {
            client,
            access_key_id: cfg.access_key_id.clone().unwrap_or_default(),
            secret_access_key: cfg.secret_access_key.clone().unwrap_or_default(),
            region: cfg
                .region
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_REGION.to_string()),
            date_style: AtomicU8::new(STYLE_UNKNOWN),
        }
    }

    /// 火山用 BCP-47，繁中 `zh-Hant`
    fn code(l: Lang) -> &'static str {
        match l {
            Lang::Auto => "",
            Lang::Zh => "zh",
            Lang::ZhTw => "zh-Hant",
            other => other.code(),
        }
    }

    fn payload(&self, req: &Request) -> String {
        let mut body = serde_json::json!({
            "TargetLanguage": Self::code(req.to),
            "TextList": [req.text.as_str()],
        });
        // 自动检测 = 完全省略 SourceLanguage 字段
        if let Some(from) = req.from.filter(|l| *l != Lang::Auto) {
            body["SourceLanguage"] = serde_json::Value::String(Self::code(from).to_string());
        }
        body.to_string()
    }

    async fn send(&self, payload: &str, date_style: u8) -> Result<util::Fetched> {
        let now = chrono::Utc::now();
        let short_date = now.format("%Y%m%d").to_string();
        let full_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let request_date = if date_style == STYLE_SHORT_DATE {
            short_date.clone()
        } else {
            full_date.clone()
        };

        let credential_scope = format!("{short_date}/{}/{SERVICE}/request", self.region);
        let canonical_headers = format!("host:{HOST}\nx-date:{full_date}\n");
        let signed_headers = "host;x-date";

        // 规范查询串必须与请求 URL 用同一份，否则签名对不上
        let canonical_query = format!("Action={ACTION}&Version={VERSION}");
        let canonical_request = format!(
            "POST\n/\n{canonical_query}\n{canonical_headers}\n{signed_headers}\n{}",
            sha256_hex(payload.as_bytes())
        );
        let string_to_sign = format!(
            "HMAC-SHA256\n{request_date}\n{credential_scope}\n{}",
            sha256_hex(canonical_request.as_bytes())
        );

        // 密钥派生链：每一级 HMAC 的输入都是原始字节，不是十六进制串
        let k_date = hmac(self.secret_access_key.as_bytes(), short_date.as_bytes());
        let k_region = hmac(&k_date, self.region.as_bytes());
        let k_service = hmac(&k_region, SERVICE.as_bytes());
        let k_signing = hmac(&k_service, b"request");
        let signature = hex::encode(hmac(&k_signing, string_to_sign.as_bytes()));

        let authorization = format!(
            "HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.access_key_id
        );

        let url = format!("https://{HOST}/?{canonical_query}");
        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-Date", full_date)
            .header("Authorization", authorization)
            .body(payload.to_string())
            .send()
            .await?;

        util::fetch("volcano", resp).await
    }
}

fn hmac(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut m = HmacSha256::new_from_slice(key).expect("HMAC-SHA256 接受任意长度密钥");
    m.update(msg);
    m.finalize().into_bytes().to_vec()
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// 从火山的 `ResponseMetadata.Error` 里取错误码
fn error_of(got: &util::Fetched) -> Option<(String, String)> {
    let err = got.json.pointer("/ResponseMetadata/Error")?;
    if err.is_null() {
        return None;
    }
    let code = err
        .get("Code")
        .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|n| n.to_string())))
        .unwrap_or_else(|| "未知".into());
    let msg = err
        .get("Message")
        .and_then(|v| v.as_str())
        .map(|s| clip(s, 300))
        .unwrap_or_else(|| "未知错误".into());
    Some((code, msg))
}

#[async_trait]
impl Engine for Volcano {
    fn id(&self) -> &'static str {
        "volcano"
    }
    fn name(&self) -> &'static str {
        "火山引擎机器翻译"
    }
    fn quota(&self) -> &'static str {
        "200万字符/月"
    }
    fn signup(&self) -> &'static str {
        "https://docs.volcengine.com/docs/MachineTranslation/Productbilling?lang=zh"
    }
    fn configured(&self) -> bool {
        !self.access_key_id.is_empty() && !self.secret_access_key.is_empty()
    }
    fn languages(&self) -> Option<&'static [Lang]> {
        None
    }

    async fn translate(&self, req: &Request) -> Result<Translation> {
        if !self.configured() {
            return Err(Error::NotConfigured("volcano"));
        }

        let payload = self.payload(req);
        let known = self.date_style.load(Ordering::Relaxed);
        let style = if known == STYLE_UNKNOWN {
            STYLE_FULL_DATETIME
        } else {
            known
        };

        let mut got = self.send(&payload, style).await?;

        // 签名形式不对时自动切换一次，并记住结论供后续调用直接使用
        let is_sig_err = error_of(&got)
            .map(|(c, _)| c.contains("Signature") || c.contains("signature"))
            .unwrap_or(false);
        if is_sig_err && known == STYLE_UNKNOWN {
            let alt = if style == STYLE_FULL_DATETIME {
                STYLE_SHORT_DATE
            } else {
                STYLE_FULL_DATETIME
            };
            match self.send(&payload, alt).await {
                Ok(retry) if error_of(&retry).is_none() => {
                    self.date_style.store(alt, Ordering::Relaxed);
                    got = retry;
                }
                _ => self.date_style.store(style, Ordering::Relaxed),
            }
        }

        if let Some((code, msg)) = error_of(&got) {
            let hint = match code.as_str() {
                "-400" => "（请求参数错误）",
                "-415" => "（语种方向不支持）",
                "-429" => "（请求过于频繁）",
                "-500" => "（翻译服务内部错误）",
                "InvalidCredential" | "InvalidAccessKey" => "（AccessKey 无效）",
                "SignatureDoesNotMatch" => "（SecretAccessKey 无效）",
                _ => "",
            };
            return Err(util::api_err("volcano", format!("{code}{hint}"), msg));
        }
        if !got.status.is_success() {
            return Err(util::api_err(
                "volcano",
                got.status.as_u16().to_string(),
                util::message_of(&got.json).unwrap_or_else(|| clip(&got.raw, 200)),
            ));
        }

        let first = got
            .json
            .pointer("/TranslationList/0")
            .ok_or_else(|| Error::bad_response("volcano", format!("响应里没有译文: {}", clip(&got.raw, 200))))?;

        let text = first
            .get("Translation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::bad_response("volcano", "TranslationList[0].Translation 缺失"))?;

        let reported = first
            .get("DetectedSourceLanguage")
            .and_then(|v| v.as_str());
        let from_lang = util::detected_or_local(&req.text, reported);

        Ok(Translation::new("volcano", text, from_lang, req.to))
    }
}
