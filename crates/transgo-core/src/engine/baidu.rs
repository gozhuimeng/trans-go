//! 百度翻译开放平台 · 通用翻译 API。
//!
//! 接口文档 <https://api.fanyi.baidu.com/doc_bd/21>
//! 免费额度：标准版 5 万字符/月；完成个人认证切到**高级版**后 100 万字符/月。
//!
//! 签名是 `MD5(appid + q + salt + key)`，`q` 取**原始未 URL 编码**的原文，编码后再算会得到 54001。

use async_trait::async_trait;
use md5::{Digest, Md5};

use super::util::{self, clip};
use super::Engine;
use crate::config::BaiduCfg;
use crate::error::{Error, Result};
use crate::lang::Lang;
use crate::types::{Request, Translation};

const ENDPOINT: &str = "https://fanyi-api.baidu.com/api/trans/vip/translate";

/// 百度标准版/高级版可用的语种（高级版也是 28 个常见语种，200+ 全量要企业尊享版）
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
    Lang::Nl,
    Lang::Pl,
    Lang::Sv,
    Lang::Da,
    Lang::Fi,
    Lang::El,
    Lang::Cs,
    Lang::Ro,
];

pub struct Baidu {
    client: reqwest::Client,
    app_id: String,
    secret: String,
}

impl Baidu {
    pub fn new(client: reqwest::Client, cfg: &BaiduCfg) -> Self {
        Baidu {
            client,
            app_id: cfg.app_id.clone().unwrap_or_default(),
            secret: cfg.secret.clone().unwrap_or_default(),
        }
    }

    /// 百度的语种代码自成一派：日语 `jp`、韩语 `kor`、繁中 `cht`
    fn code(l: Lang) -> &'static str {
        match l {
            Lang::Auto => "auto",
            Lang::Zh => "zh",
            Lang::ZhTw => "cht",
            Lang::En => "en",
            Lang::Ja => "jp",
            Lang::Ko => "kor",
            Lang::Fr => "fra",
            Lang::De => "de",
            Lang::Es => "spa",
            Lang::Pt => "pt",
            Lang::It => "it",
            Lang::Ru => "ru",
            Lang::Ar => "ara",
            Lang::Th => "th",
            Lang::Vi => "vie",
            Lang::Nl => "nl",
            Lang::Pl => "pl",
            Lang::Sv => "swe",
            Lang::Da => "dan",
            Lang::Fi => "fin",
            Lang::El => "el",
            Lang::Cs => "cze",
            Lang::Ro => "rom",
            other => other.code(),
        }
    }
}

fn md5_hex(s: &str) -> String {
    let mut h = Md5::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

#[async_trait]
impl Engine for Baidu {
    fn id(&self) -> &'static str {
        "baidu"
    }
    fn name(&self) -> &'static str {
        "百度翻译"
    }
    fn quota(&self) -> &'static str {
        "标准版 5万/月；实名认证后高级版 100万/月"
    }
    fn signup(&self) -> &'static str {
        "https://fanyi-api.baidu.com/api/trans/product/desktop"
    }
    fn configured(&self) -> bool {
        !self.app_id.is_empty() && !self.secret.is_empty()
    }
    fn languages(&self) -> Option<&'static [Lang]> {
        Some(SUPPORTED)
    }

    async fn translate(&self, req: &Request) -> Result<Translation> {
        if !self.configured() {
            return Err(Error::NotConfigured("baidu"));
        }

        let salt = uuid::Uuid::new_v4().to_string();
        // 关键：sign 用原始 q，不带 URL 编码
        let sign = md5_hex(&format!("{}{}{}{}", self.app_id, req.text, salt, self.secret));

        let from = req.from.unwrap_or(Lang::Auto);
        let resp = self
            .client
            .post(ENDPOINT)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[
                ("q", req.text.as_str()),
                ("from", Self::code(from)),
                ("to", Self::code(req.to)),
                ("appid", self.app_id.as_str()),
                ("salt", &salt),
                ("sign", &sign),
            ])
            .send()
            .await?;
        let got = util::fetch("baidu", resp).await?;

        // 百度把错误也放在 HTTP 200 里，error_code 可能是字符串也可能是整数
        if let Some(code) = got.json.get("error_code").filter(|v| !v.is_null()) {
            let code = code
                .as_str()
                .map(|s| s.to_string())
                .or_else(|| code.as_i64().map(|n| n.to_string()))
                .unwrap_or_else(|| "未知".into());
            let msg = got
                .json
                .get("error_msg")
                .and_then(|v| v.as_str())
                .map(clip_300)
                .unwrap_or_else(|| "未知错误".into());
            let hint = match code.as_str() {
                "54001" => "（签名错误：检查 appid / 密钥，注意 q 不能预先 URL 编码）",
                "52003" => "（未授权用户，检查 appid）",
                "54003" | "54005" => "（访问频率受限）",
                "54004" => "（账户余额不足）",
                "58001" => "（译文语言方向不支持）",
                "90107" => "（实名认证未通过或未生效）",
                _ => "",
            };
            return Err(util::api_err("baidu", format!("{code}{hint}"), msg));
        }

        let entries = got
            .json
            .pointer("/trans_result")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::bad_response("baidu", format!("响应里没有译文: {}", clip(&got.raw, 200))))?;

        // 多行 q 会产生多条 trans_result，按行拼回去
        let text = entries
            .iter()
            .filter_map(|e| e.get("dst").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
            .join("\n");

        if text.is_empty() {
            return Err(Error::bad_response("baidu", "trans_result 为空"));
        }

        let reported = got
            .json
            .get("from")
            .and_then(|v| v.as_str());
        let from_lang = util::detected_or_local(&req.text, reported);

        Ok(Translation::new("baidu", text, from_lang, req.to))
    }
}

fn clip_300(s: &str) -> String {
    clip(s, 300)
}
