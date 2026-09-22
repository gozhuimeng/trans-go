//! MyMemory · 免注册的翻译记忆库接口，做零配置兜底。
//!
//! 接口文档 <https://mymemory.translated.net/doc/spec.php>
//! 免费额度：匿名 5 千字符/天，留邮箱后 5 万字符/天。

use async_trait::async_trait;

use super::util::{self, clip, unescape_html};
use super::Engine;
use crate::config::MyMemoryCfg;
use crate::error::{Error, Result};
use crate::lang::Lang;
use crate::types::{Request, Translation};

const ENDPOINT: &str = "https://api.mymemory.translated.net/get";

pub struct MyMemory {
    client: reqwest::Client,
    email: Option<String>,
}

impl MyMemory {
    pub fn new(client: reqwest::Client, cfg: &MyMemoryCfg) -> Self {
        MyMemory {
            client,
            email: cfg.email.clone().filter(|s| !s.is_empty()),
        }
    }

    /// MyMemory 用带地区的代码，简中是 `zh-CN`
    fn code(l: Lang) -> &'static str {
        match l {
            Lang::Auto => "Autodetect",
            Lang::Zh => "zh-CN",
            Lang::ZhTw => "zh-TW",
            Lang::En => "en-GB",
            other => other.code(),
        }
    }
}

#[async_trait]
impl Engine for MyMemory {
    fn id(&self) -> &'static str {
        "mymemory"
    }
    fn name(&self) -> &'static str {
        "MyMemory"
    }
    fn quota(&self) -> &'static str {
        "5千字符/天（留邮箱 5万）"
    }
    fn signup(&self) -> &'static str {
        "https://mymemory.translated.net/doc/usagelimits.php"
    }
    fn configured(&self) -> bool {
        true // 免注册
    }
    fn languages(&self) -> Option<&'static [Lang]> {
        None // 收得很宽，不做白名单限制
    }

    async fn translate(&self, req: &Request) -> Result<Translation> {
        // 源语未知时用 `Autodetect`，**不要**用本地字符类别探测的结果冒充具体源语。
        //
        // 实测对比（zh-CN -> en-GB）：
        //   "今天天气不错"  具体源语 -> "today's Minnesota ideas"   (翻译记忆库模糊命中噪声条目)
        //                  Autodetect -> "It's a nice day today"    (走机翻，正确)
        //   "知识就是力量"  两者都对，具体源语那条多给几个备选。
        //
        // 给了具体源语，MyMemory 会优先查翻译记忆库并把命中条目当作主译文，命中质量是抽奖；
        // `Autodetect` 查不到记忆库就走机翻，反而稳定。主译文可靠性 > 备选译文数量。
        // 显式传 `-f` 时仍走记忆库路径，那是用户主动要的。
        let from = req.from.unwrap_or(Lang::Auto);
        let langpair = format!("{}|{}", Self::code(from), Self::code(req.to));

        let mut query = vec![("q", req.text.clone()), ("langpair", langpair)];
        if let Some(mail) = &self.email {
            query.push(("de", mail.clone()));
        }

        let resp = self.client.get(ENDPOINT).query(&query).send().await?;
        let got = util::fetch("mymemory", resp).await?;

        // MyMemory 用 HTTP 200 + responseStatus 报错，错误信息可能塞在 responseData.translatedText 里
        let status = got
            .json
            .get("responseStatus")
            .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
            .unwrap_or(200);
        if status != 200 {
            let msg = util::message_of(&got.json)
                .or_else(|| {
                    got.json
                        .pointer("/responseData/translatedText")
                        .and_then(|v| v.as_str())
                        .map(clip_300)
                })
                .unwrap_or_else(|| "未知错误".into());
            return Err(util::api_err("mymemory", status.to_string(), msg));
        }

        let matches = got
            .json
            .pointer("/matches")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // `responseData.translatedText` 是文档指定的主译文字段。
        // `matches[]` 是翻译记忆库命中列表，同一段原文可能同时命中多条记录（比如 "hello"
        // 既有 "你好" 也有 "您好"），而且排序在不同请求间会漂移，所以只能拿它当备选。
        let primary = got
            .json
            .pointer("/responseData/translatedText")
            .and_then(|v| v.as_str())
            .map(unescape_html)
            .filter(|s| !s.is_empty())
            .or_else(|| {
                matches
                    .first()
                    .and_then(|m| m.get("translation"))
                    .and_then(|v| v.as_str())
                    .map(unescape_html)
                    .filter(|s| !s.is_empty())
            });

        let text = primary.ok_or_else(|| {
            Error::bad_response("mymemory", format!("响应里没有译文: {}", clip(&got.raw, 200)))
        })?;

        // 源语种只有 matches 里带，主响应没这个字段
        let reported = matches
            .first()
            .and_then(|m| m.get("source"))
            .and_then(|v| v.as_str());
        let from_lang = util::detected_or_local(&req.text, reported);

        // 其余 matches 是翻译记忆库里的备选译文，去重后展示
        let mut alternatives = Vec::new();
        for m in &matches {
            if let Some(alt) = m.get("translation").and_then(|v| v.as_str()) {
                let alt = unescape_html(alt);
                if !alt.is_empty() && alt != text && !alternatives.contains(&alt) {
                    alternatives.push(alt);
                }
            }
            if alternatives.len() >= 5 {
                break;
            }
        }

        Ok(Translation::new("mymemory", text, from_lang, req.to).with_alternatives(alternatives))
    }
}

fn clip_300(s: &str) -> String {
    clip(s, 300)
}
