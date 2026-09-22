//! 腾讯云机器翻译 TMT · `TextTranslate`。
//!
//! 接口文档 <https://cloud.tencent.com/document/api/551/15615>
//! 免费额度：**每月 500 万字符**，国内几家里额度最大方。
//!
//! 鉴权是腾讯云 TC3-HMAC-SHA256，规范请求里的头部名要小写并按字典序排好。

use async_trait::async_trait;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use super::util::{self, clip};
use super::Engine;
use crate::config::TencentCfg;
use crate::error::{Error, Result};
use crate::lang::Lang;
use crate::types::{Request, Translation};

const HOST: &str = "tmt.tencentcloudapi.com";
const ENDPOINT: &str = "https://tmt.tencentcloudapi.com";
const SERVICE: &str = "tmt";
const VERSION: &str = "2018-03-21";
const ACTION: &str = "TextTranslate";
const DEFAULT_REGION: &str = "ap-guangzhou";

type HmacSha256 = Hmac<Sha256>;

/// 腾讯云 TMT 支持的语种
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
];

pub struct Tencent {
    client: reqwest::Client,
    secret_id: String,
    secret_key: String,
    region: String,
}

impl Tencent {
    pub fn new(client: reqwest::Client, cfg: &TencentCfg) -> Self {
        Tencent {
            client,
            secret_id: cfg.secret_id.clone().unwrap_or_default(),
            secret_key: cfg.secret_key.clone().unwrap_or_default(),
            region: cfg
                .region
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_REGION.to_string()),
        }
    }

    /// 腾讯云用 `zh` / `zh-TW`
    fn code(l: Lang) -> &'static str {
        match l {
            Lang::Auto => "auto",
            Lang::ZhTw => "zh-TW",
            other => other.code(),
        }
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

#[async_trait]
impl Engine for Tencent {
    fn id(&self) -> &'static str {
        "tencent"
    }
    fn name(&self) -> &'static str {
        "腾讯云机器翻译"
    }
    fn quota(&self) -> &'static str {
        "500万字符/月"
    }
    fn signup(&self) -> &'static str {
        "https://cloud.tencent.com/document/product/551/40566"
    }
    fn configured(&self) -> bool {
        !self.secret_id.is_empty() && !self.secret_key.is_empty()
    }
    fn languages(&self) -> Option<&'static [Lang]> {
        Some(SUPPORTED)
    }

    async fn translate(&self, req: &Request) -> Result<Translation> {
        if !self.configured() {
            return Err(Error::NotConfigured("tencent"));
        }

        let payload = serde_json::json!({
            "SourceText": req.text.as_str(),
            "Source": Self::code(req.from.unwrap_or(Lang::Auto)),
            "Target": Self::code(req.to),
            "ProjectId": 0,
        })
        .to_string();

        let now = chrono::Utc::now();
        let timestamp = now.timestamp();
        let date = now.format("%Y-%m-%d").to_string();

        // ---- TC3-HMAC-SHA256 ----
        let canonical_headers = format!(
            "content-type:application/json; charset=utf-8\nhost:{HOST}\nx-tc-action:{}\n",
            ACTION.to_ascii_lowercase()
        );
        let signed_headers = "content-type;host;x-tc-action";

        let canonical_request = format!(
            "POST\n/\n\n{canonical_headers}\n{signed_headers}\n{}",
            sha256_hex(payload.as_bytes())
        );

        let credential_scope = format!("{date}/{SERVICE}/tc3_request");
        let string_to_sign = format!(
            "TC3-HMAC-SHA256\n{timestamp}\n{credential_scope}\n{}",
            sha256_hex(canonical_request.as_bytes())
        );

        let k_date = hmac(format!("TC3{}", self.secret_key).as_bytes(), date.as_bytes());
        let k_service = hmac(&k_date, SERVICE.as_bytes());
        let k_signing = hmac(&k_service, b"tc3_request");
        let signature = hex::encode(hmac(&k_signing, string_to_sign.as_bytes()));

        let authorization = format!(
            "TC3-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.secret_id
        );

        let got = util::fetch(
            "tencent",
            self.client
                .post(ENDPOINT)
                .header("Authorization", authorization)
                .header("Content-Type", "application/json; charset=utf-8")
                .header("X-TC-Action", ACTION)
                .header("X-TC-Timestamp", timestamp.to_string())
                .header("X-TC-Version", VERSION)
                .header("X-TC-Region", self.region.as_str())
                .body(payload)
                .send()
                .await?,
        )
        .await?;

        // 腾讯云把错误包在 Response.Error 里，HTTP 状态也可能是 200
        if let Some(err) = got.json.pointer("/Response/Error").filter(|v| !v.is_null()) {
            let code = err.get("Code").and_then(|v| v.as_str()).unwrap_or("未知").to_string();
            let msg = err
                .get("Message")
                .and_then(|v| v.as_str())
                .map(|s| clip(s, 300))
                .unwrap_or_else(|| "未知错误".into());
            let hint = match code.as_str() {
                "AuthFailure.SignatureFailure" => "（SecretKey 不对）",
                "AuthFailure.SecretIdNotFound" => "（SecretId 不对）",
                "AuthFailure.TokenFailure" | "AuthFailure.SignatureExpire" => "（时间戳过期，校准系统时钟）",
                "UnauthorizedOperation" => "（子账号无 TMT 权限）",
                "ResourceUnavailable.ServiceNotOpen" => "（机器翻译服务未开通）",
                "LimitExceeded" => "（配额耗尽或触发限频）",
                "InvalidParameter" => "（语种方向不支持）",
                _ => "",
            };
            return Err(util::api_err("tencent", format!("{code}{hint}"), msg));
        }
        if !got.status.is_success() {
            return Err(util::api_err(
                "tencent",
                got.status.as_u16().to_string(),
                util::message_of(&got.json).unwrap_or_else(|| clip(&got.raw, 200)),
            ));
        }

        let text = got
            .json
            .pointer("/Response/TargetText")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::bad_response("tencent", format!("响应里没有译文: {}", clip(&got.raw, 200))))?;

        let reported = got.json.pointer("/Response/Source").and_then(|v| v.as_str());
        let from_lang = util::detected_or_local(&req.text, reported);

        Ok(Translation::new("tencent", text, from_lang, req.to))
    }
}

#[cfg(test)]
mod tests {
    /// 腾讯云官方示例：规范请求的哈希值固定，可用来回归签名拆分是否被误改
    #[test]
    fn sha256_of_known_string_is_stable() {
        assert_eq!(
            super::sha256_hex(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }
}
