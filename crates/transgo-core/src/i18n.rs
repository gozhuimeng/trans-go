//! 界面语言：一套文案两种语言，全局开关。
//!
//! 短期内只有中文、英文两档，由 `ui.lang`（`TRANSGO_UI_LANG`）设置。
//! 启动早期调用 [`set_ui_lang`]，之后所有字符串经 [`s`] 取对应语言。
//!
//! 用全局而不是层层传参：文案散落在报错、帮助、引擎描述、GUI 各处，
//! 短期内全局开关是最省事也够用的形态。

use std::sync::atomic::{AtomicBool, Ordering};

/// 界面语言
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLang {
    Zh,
    En,
}

/// false = 中文，true = 英文
static EN: AtomicBool = AtomicBool::new(false);

/// 设置界面语言。启动时调用一次即可。
pub fn set_ui_lang(lang: UiLang) {
    EN.store(lang == UiLang::En, Ordering::Relaxed);
}

/// 当前界面语言
pub fn ui_lang() -> UiLang {
    if EN.load(Ordering::Relaxed) {
        UiLang::En
    } else {
        UiLang::Zh
    }
}

/// 取当前语言的文案：`s("中文", "English")`
pub fn s(zh: &'static str, en: &'static str) -> &'static str {
    match ui_lang() {
        UiLang::Zh => zh,
        UiLang::En => en,
    }
}

/// 解析 `ui.lang` 配置。空 / 未设置 = 默认 `zh`；`zh` / `en` 之外的值返回非法取值本身，
/// 由调用方格式化错误（此时语言尚未切换，按默认中文出文案）。
pub fn ui_lang_from(raw: Option<&str>) -> Result<UiLang, String> {
    match raw.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "" | "zh" => Ok(UiLang::Zh),
        "en" => Ok(UiLang::En),
        other => Err(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_language_switches_strings() {
        set_ui_lang(UiLang::Zh);
        assert_eq!(s("原文", "Source"), "原文");
        set_ui_lang(UiLang::En);
        assert_eq!(s("原文", "Source"), "Source");
        set_ui_lang(UiLang::Zh);
    }

    #[test]
    fn ui_lang_parsing() {
        assert_eq!(ui_lang_from(None), Ok(UiLang::Zh));
        assert_eq!(ui_lang_from(Some("zh")), Ok(UiLang::Zh));
        assert_eq!(ui_lang_from(Some("EN")), Ok(UiLang::En));
        assert!(ui_lang_from(Some("jp")).is_err());
    }
}
