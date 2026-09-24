//! 任意 OpenAI 兼容的 `/chat/completions` 接口。
//!
//! OpenAI / DeepSeek / 硅基流动 / Groq / OpenRouter / 本地 Ollama 都能用。
//! 翻译质量上限最高，也能顺便润色、统一术语、加注音标。

use async_trait::async_trait;

use super::util::{self, clip};
use super::Engine;
use crate::config::LlmCfg;
use crate::error::{Error, Result};
use crate::lang::Lang;
use crate::types::{Request, Translation};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_TEMPERATURE: f64 = 0.2;

const DEFAULT_PROMPT: &str = "You are a professional translator. \
Translate the user's text into the requested target language. \
Detect the source language yourself if it is not given.\n\
Rules:\n\
- Output ONLY the translation. No explanations, no notes, no romanization, no quotation marks.\n\
- Preserve paragraph breaks, list markers, markdown formatting and code blocks verbatim.\n\
- Preserve the tone, register and terminology of the original.\n\
- If the text is already in the target language, output it unchanged.";

pub struct Llm {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
    system_prompt: String,
    temperature: f64,
}

impl Llm {
    pub fn new(client: reqwest::Client, cfg: &LlmCfg) -> Self {
        Llm {
            client,
            api_key: cfg.api_key.clone().unwrap_or_default(),
            base_url: cfg
                .base_url
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            model: cfg
                .model
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            system_prompt: cfg
                .system_prompt
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_PROMPT.to_string()),
            temperature: cfg.temperature.unwrap_or(DEFAULT_TEMPERATURE),
        }
    }
}

#[async_trait]
impl Engine for Llm {
    fn id(&self) -> &'static str {
        "llm"
    }
    fn name(&self) -> &'static str {
        crate::i18n::s("LLM (OpenAI 兼容)", "LLM (OpenAI compatible)")
    }
    fn quota(&self) -> &'static str {
        crate::i18n::s("取决于所选服务商", "Depends on the provider")
    }
    fn signup(&self) -> &'static str {
        "https://platform.openai.com/ (或 DeepSeek / 硅基流动 / 本地 Ollama)"
    }
    fn configured(&self) -> bool {
        // 本地与内网自建服务不需要 Key（Ollama、vLLM、局域网翻译模型）
        !self.model.is_empty() && (!self.api_key.is_empty() || is_keyless_host(&self.base_url))
    }
    fn languages(&self) -> Option<&'static [Lang]> {
        None // 没有语种限制
    }

    async fn translate(&self, req: &Request) -> Result<Translation> {
        if !self.configured() {
            return Err(Error::NotConfigured("llm"));
        }

        let from = match req.from {
            Some(l) if l != Lang::Auto => format!("source language: {}", l.name_en()),
            _ => "source language: auto-detect".to_string(),
        };
        let user = format!(
            "target language: {}\n{from}\n\n---\n{}",
            req.to.name_en(),
            req.text
        );

        let body = serde_json::json!({
            "model": self.model,
            "temperature": self.temperature,
            "messages": [
                { "role": "system", "content": self.system_prompt },
                { "role": "user", "content": user },
            ],
        });

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let mut call = self.client.post(&url).json(&body);
        if !self.api_key.is_empty() {
            call = call.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let got = util::fetch("llm", call.send().await?).await?;
        if !got.status.is_success() {
            let msg = util::message_of(&got.json).unwrap_or_else(|| clip(&got.raw, 300));
            let hint = match got.status.as_u16() {
                401 => "（API Key 无效）",
                402 => "（余额不足）",
                404 => "（model 名称不对，或 base_url 少了 /v1）",
                429 => "（速率受限）",
                _ => "",
            };
            return Err(util::api_err("llm", format!("{}{hint}", got.status.as_u16()), msg));
        }

        let text = got
            .json
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                Error::bad_response("llm", format!("响应里没有译文: {}", clip(&got.raw, 300)))
            })?;

        // 有些模型会自作主张包一层引号
        let text = strip_wrapping_quotes(&text);

        let from_lang = util::detected_or_local(&req.text, None);
        Ok(Translation::new("llm", text, from_lang, req.to))
    }
}

/// 本地与内网自建的 OpenAI 兼容服务一般不要 Key：localhost、回环、私有网段。
/// 公网地址一律要求 Key（那些服务都是租的，没 Key 不可信）。
fn is_keyless_host(base_url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(base_url) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v4)) => {
            v4.is_loopback()
                || v4.octets()[0] == 10
                || (v4.octets()[0] == 172 && (16..=31).contains(&v4.octets()[1]))
                || (v4.octets()[0] == 192 && v4.octets()[1] == 168)
        }
        Ok(std::net::IpAddr::V6(v6)) => v6.is_loopback(),
        Err(_) => false,
    }
}

/// 清掉模型的加戏：开头回显的分隔线、外层引号（文本内部的引号保留）
fn strip_wrapping_quotes(s: &str) -> String {
    let mut t = s.trim();
    // 小模型偶尔把提示词里的分隔线原样吐回来。只剥开头第一行，
    // 且后面得真有内容；整段就一个 --- 时不动它
    if let Some(rest) = t.strip_prefix("---") {
        let after_break = rest
            .strip_prefix('\n')
            .or_else(|| rest.strip_prefix("\r\n"));
        if let Some(content) = after_break {
            if !content.trim().is_empty() {
                t = content.trim_start();
            }
        }
    }
    for (o, c) in [('"', '"'), ('「', '」'), ('『', '』')] {
        if t.len() >= 2 && t.starts_with(o) && t.ends_with(c) {
            let inner = &t[o.len_utf8()..t.len() - c.len_utf8()];
            if !inner.contains(o) {
                return inner.trim().to_string();
            }
        }
    }
    t.to_string()
}

#[cfg(test)]
mod tests {
    use super::{is_keyless_host, strip_wrapping_quotes};

    #[test]
    fn strips_outer_quotes_only() {
        assert_eq!(strip_wrapping_quotes("\"你好\""), "你好");
        assert_eq!(strip_wrapping_quotes("他说\"你好\""), "他说\"你好\"");
        assert_eq!(strip_wrapping_quotes("「注釈」"), "注釈");
    }

    #[test]
    fn strips_echoed_separator() {
        assert_eq!(strip_wrapping_quotes("---\n早起的鸟能抓到虫"), "早起的鸟能抓到虫");
        assert_eq!(strip_wrapping_quotes("---\r\n你好"), "你好");
        // 正文里的分隔线不动，单独一行的 --- 也不当回显
        assert_eq!(strip_wrapping_quotes("---"), "---");
        assert_eq!(strip_wrapping_quotes("第一行\n---\n第二行"), "第一行\n---\n第二行");
    }

    #[test]
    fn keyless_covers_local_and_lan_only() {
        // 本地回环与常见自建服务
        assert!(is_keyless_host("http://localhost:11434/v1"));
        assert!(is_keyless_host("http://127.0.0.1:8080/v1"));
        assert!(is_keyless_host("http://[::1]:8080/v1"));
        // 内网自建服务（局域网翻译模型就在这类地址上）
        assert!(is_keyless_host("http://10.0.0.144:8080/v1"));
        assert!(is_keyless_host("http://192.168.1.5:8000/v1"));
        assert!(is_keyless_host("http://172.16.0.1:8000/v1"));
        // 公网服务必须有 Key
        assert!(!is_keyless_host("https://api.openai.com/v1"));
        assert!(!is_keyless_host("http://172.32.0.1:8000/v1"));
        assert!(!is_keyless_host("不是 URL"));
    }
}
