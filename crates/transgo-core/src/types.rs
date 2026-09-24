//! 翻译请求与结果。

use serde::{Deserialize, Serialize};

use crate::Lang;

/// 一次翻译请求。`to` 已解析为具体语种，不会是 [`Lang::Auto`]。
#[derive(Debug, Clone)]
pub struct Request {
    pub text: String,
    /// 源语种；`None` 表示交给引擎自动检测
    pub from: Option<Lang>,
    /// 目标语种
    pub to: Lang,
    /// 翻译指令：告诉引擎「怎么译」（文风、场景之类），如「采用意译」。
    /// 仅支持的引擎读取，不认的忽略
    pub instruction: Option<String>,
}

impl Request {
    /// 组装请求。`to` 为 `None`/`Auto` 时自动决策：**中文译英文，其余一切译中文**。
    ///
    /// 判定依据是 [`crate::detect::is_chinese`]（看整段字符构成，不是首字）。
    /// 用户随时可以用 `-t` 覆盖。
    pub fn new(text: impl Into<String>, from: Option<Lang>, to: Option<Lang>) -> Request {
        let text = text.into();
        let to = match to {
            Some(l) if l != Lang::Auto => l,
            _ => crate::detect::auto_target(&text),
        };
        Request { text, from, to, instruction: None }
    }

    /// 附上翻译指令。空串与 `None` 等价，表示用引擎默认风格。
    pub fn with_instruction(mut self, instruction: Option<String>) -> Request {
        self.instruction = instruction.filter(|s| !s.trim().is_empty());
        self
    }
}

/// 一次翻译的结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Translation {
    /// 译文
    pub text: String,
    /// 源语种（引擎自动检测的结果；引擎不返回时退化为本地探测）
    pub from: Lang,
    /// 目标语种
    pub to: Lang,
    /// 实际使用的引擎 id
    pub engine: String,
    /// 备选译文
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<String>,
}

impl Translation {
    pub fn new(engine: impl Into<String>, text: impl Into<String>, from: Lang, to: Lang) -> Self {
        Translation {
            text: text.into(),
            from,
            to,
            engine: engine.into(),
            alternatives: Vec::new(),
        }
    }

    pub fn with_alternatives(mut self, alts: Vec<String>) -> Self {
        self.alternatives = alts;
        self
    }
}
