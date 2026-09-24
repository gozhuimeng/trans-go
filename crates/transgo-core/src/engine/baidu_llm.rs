//! 百度翻译开放平台 · 大模型文本翻译 API（`model_type = llm`）。
//!
//! 接口文档 <https://fanyi-api.baidu.com/doc/21>
//!
//! 与 [`super::baidu`] 的通用翻译 API 是两个独立接口、两套免费额度：
//! 这边是认证后自动发放的一次性 100 万字符测试额度，不按月重置；
//! 用完后 49 元/百万字符扣余额，余额不足报 54004，次日结算，无自动充值。
//!
//! 鉴权两种都行：Bearer API Key（配置 `baidu.api_key`），
//! 或与通用翻译相同的 `MD5(appid + q + salt + key)` 签名。

use async_trait::async_trait;

use super::baidu::{md5_hex, Baidu, SUPPORTED};
use super::util::{self, clip};
use super::Engine;
use crate::config::BaiduCfg;
use crate::error::{Error, Result};
use crate::lang::Lang;
use crate::types::{Request, Translation};

const ENDPOINT: &str = "https://fanyi-api.baidu.com/ait/api/aiTextTranslate";

pub struct BaiduLlm {
    client: reqwest::Client,
    app_id: String,
    secret: String,
    api_key: String,
    /// 默认翻译指令，请求里带了就用请求的
    reference: String,
    /// 术语库干预开关
    need_intervene: bool,
}

impl BaiduLlm {
    pub fn new(client: reqwest::Client, cfg: &BaiduCfg) -> Self {
        BaiduLlm {
            client,
            app_id: cfg.app_id.clone().unwrap_or_default(),
            secret: cfg.secret.clone().unwrap_or_default(),
            api_key: cfg.api_key.clone().unwrap_or_default(),
            reference: cfg.reference.clone().unwrap_or_default(),
            need_intervene: cfg.need_intervene.unwrap_or(false),
        }
    }
}

#[async_trait]
impl Engine for BaiduLlm {
    fn id(&self) -> &'static str {
        "baidu-llm"
    }
    fn name(&self) -> &'static str {
        crate::i18n::s("百度大模型翻译", "Baidu LLM Translate")
    }
    fn quota(&self) -> &'static str {
        crate::i18n::s("认证后一次性 100 万字符测试额度，不按月重置", "One-time 1 M-character test quota after identity verification, no monthly reset")
    }
    fn signup(&self) -> &'static str {
        "https://fanyi-api.baidu.com/choose"
    }
    fn configured(&self) -> bool {
        !self.app_id.is_empty() && (!self.api_key.is_empty() || !self.secret.is_empty())
    }
    fn languages(&self) -> Option<&'static [Lang]> {
        Some(SUPPORTED)
    }

    async fn translate(&self, req: &Request) -> Result<Translation> {
        if !self.configured() {
            return Err(Error::NotConfigured("baidu-llm"));
        }

        let from = req.from.unwrap_or(Lang::Auto);
        let mut body = serde_json::json!({
            "appid": self.app_id,
            "q": req.text,
            "from": Baidu::code(from),
            "to": Baidu::code(req.to),
            "model_type": "llm",
        });

        // 翻译指令：请求里带的优先，其次用配置的默认指令。上限 500 字符（超出报 59002）
        let instruction = req
            .instruction
            .as_deref()
            .unwrap_or(self.reference.as_str())
            .trim();
        if !instruction.is_empty() {
            body["reference"] = serde_json::Value::String(instruction.to_string());
        }
        // 术语库干预：术语表在控制台「我的术语库」维护，功能本身不额外收费
        if self.need_intervene {
            body["needIntervene"] = serde_json::Value::from(1);
        }

        let mut call = self.client.post(ENDPOINT).header("Content-Type", "application/json");
        if self.api_key.is_empty() {
            // 关键：sign 用原始 q，不带 URL 编码。
            // salt 在这个接口必须是 int64 数字（JSON 体里传字符串或超 int64 的数都会被
            // 网关以 53001 拒绝），MD5 拼接时用它的十进制形式
            let salt = (uuid::Uuid::new_v4().as_u128() as u64) & i64::MAX as u64;
            let sign = md5_hex(&format!("{}{}{}{}", self.app_id, req.text, salt, self.secret));
            body["salt"] = serde_json::Value::from(salt);
            body["sign"] = serde_json::Value::String(sign);
        } else {
            call = call.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let resp = call.body(body.to_string()).send().await?;
        let got = util::fetch("baidu-llm", resp).await?;

        // 百度把错误也放在 HTTP 200 里，error_code 可能是字符串也可能是整数
        if let Some(val) = got.json.get("error_code").filter(|v| !v.is_null()) {
            let err_code = val
                .as_str()
                .map(|s| s.to_string())
                .or_else(|| val.as_i64().map(|n| n.to_string()))
                .unwrap_or_else(|| "未知".into());
            let msg = got
                .json
                .get("error_msg")
                .and_then(|v| v.as_str())
                .map(clip_300)
                .unwrap_or_else(|| "未知错误".into());
            let hint = match err_code.as_str() {
                "52001" => "（请求超时，检查 q 参数与语种方向）",
                "52002" => "（系统错误，重试）",
                "52003" => "（未授权用户，检查 appid / api_key，以及大模型文本翻译服务是否开通）",
                "54000" => "（必填参数为空）",
                "54001" => "（签名或 token 错误：检查 api_key，或 appid / 密钥是否抄错）",
                "54003" => "（访问频率受限）",
                "54004" => "（账户余额不足，测试额度用完且余额为 0）",
                "54005" => "（长文本请求过频，3 秒后重试）",
                "58000" => "（客户端 IP 非法，检查开发者信息页的服务器 IP）",
                "58001" => "（译文语言方向不支持）",
                "58002" => "（服务已关闭，去控制台开启）",
                "58003" => "（IP 被封禁，同一 IP 当日用了多个 appid，次日解封）",
                "58004" => "（模型参数错误）",
                "59003" => "（请求文本过长，单次上限 6000 字符）",
                "59004" => "（QPS 超限）",
                "90107" => "（实名认证未通过或未生效）",
                "20003" => "（请求内容存在安全风险）",
                _ => "",
            };
            return Err(util::api_err("baidu-llm", format!("{err_code}{hint}"), msg));
        }

        let entries = got
            .json
            .pointer("/trans_result")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::bad_response("baidu-llm", format!("响应里没有译文: {}", clip(&got.raw, 200))))?;

        // 多行 q 会产生多条 trans_result，按行拼回去
        let text = entries
            .iter()
            .filter_map(|e| e.get("dst").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
            .join("\n");

        if text.is_empty() {
            return Err(Error::bad_response("baidu-llm", "trans_result 为空"));
        }

        let reported = got
            .json
            .get("from")
            .and_then(|v| v.as_str());
        let from_lang = util::detected_or_local(&req.text, reported);

        Ok(Translation::new("baidu-llm", text, from_lang, req.to))
    }
}

fn clip_300(s: &str) -> String {
    clip(s, 300)
}
