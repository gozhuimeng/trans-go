//! 目标语决策：只回答「原文是不是中文」这一个问题。
//!
//! 默认规则就一条，没有别的分支：
//!
//! ```text
//! 是中文     → 译成英文
//! 不是中文   → 译成中文
//! ```
//!
//! 判定看的是**整段文本的字符构成比例**，不是首字 —— 否则
//! 「iPhone手机很好用」这种以英文单词开头的中文会被判成英文。
//!
//! 这个结果只用于决定「往哪个方向译」。真正的源语种由远端引擎自动检测，
//! 用户也可以用 `-f` / `-t` 在出问题时手动覆盖。

use crate::Lang;

/// 汉字占「字母类字符」的比例达到这个值就认为是中文。
///
/// 取 1/3 而不是 1/2：中文句子里夹英文专有名词太常见了（「iPhone手机很好用」
/// 「GDP增长了5个百分点」），汉字数未必过半，但它们毫无疑问是中文。
/// 反过来，英文句子里偶尔夹一两个汉字词（「那个 meeting 的议程」）也不该翻转判定。
const CHINESE_RATIO: (usize, usize) = (1, 3);

/// 原文是不是中文。
///
/// 数字、标点、空白**不计入统计** —— 它们在任何语言里长得都一样，掺进来只会稀释比例，
/// 让判定更靠近阈值、更不稳定。
pub fn is_chinese(text: &str) -> bool {
    let mut han = 0usize;
    let mut other_letters = 0usize;
    let mut kana_hangul = 0usize;

    for ch in text.chars() {
        let cp = ch as u32;
        match cp {
            // CJK 统一表意文字（含扩展 A 与兼容区）
            0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF => han += 1,

            // 假名 / 谚文 —— 日韩文专属字符
            0x1100..=0x11FF | 0x3040..=0x30FF | 0x3130..=0x318F | 0x31F0..=0x31FF | 0xAC00..=0xD7A3 => {
                kana_hangul += 1
            }

            // 其余语素文字与字母
            0x0041..=0x005A | 0x0061..=0x007A | 0x00C0..=0x024F // 拉丁
            | 0x0370..=0x03FF // 希腊
            | 0x0400..=0x052F // 西里尔
            | 0x0600..=0x06FF | 0x0750..=0x077F | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF // 阿拉伯
            | 0x0900..=0x097F // 天城文
            | 0x0590..=0x05FF // 希伯来
            | 0x0E00..=0x0E7F // 泰文
            => other_letters += 1,

            // 数字、标点、空白、符号：不计
            _ => {}
        }
    }

    // 假名/谚文是日韩文的专属字符，中文里一个都不会有。
    //
    // 已知局限：中文句子里夹一整个片假名词（「我很喜欢カタカナ」）会被判成日文。
    // 这两种情况在字符统计上完全一样（都是汉字与假名各半），区分不了。
    // 优先照顾日文，是因为「日译中」是常态需求，反过来那种写法很少见。
    if kana_hangul > 0 {
        return false;
    }

    // 没有汉字，显然不是中文
    if han == 0 {
        return false;
    }

    // 纯汉字 + 数字/标点（例如「2024年GDP增长5%」里的汉字够不上也仍是中文），
    // 只要有汉字且没有其他字母，直接判中文
    if other_letters == 0 {
        return true;
    }

    han * CHINESE_RATIO.1 >= (han + other_letters) * CHINESE_RATIO.0
}

/// 目标语决策：中文译英文，其余一切译中文。
pub fn auto_target(text: &str) -> Lang {
    if is_chinese(text) {
        Lang::En
    } else {
        Lang::Zh
    }
}

/// 粗略的语种标注，**只用于展示**。
///
/// 远端引擎有的不回传检测到的源语种（比如 Azure），这时用它兜底给 `-v` / `--json`
/// 里的 `from` 字段填个大概值。它不影响译文方向 —— 方向只看 [`is_chinese`]。
pub fn detect(text: &str) -> Lang {
    for ch in text.chars() {
        let cp = ch as u32;
        match cp {
            0x3040..=0x30FF | 0x31F0..=0x31FF => return Lang::Ja,
            0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7A3 => return Lang::Ko,
            _ => {}
        }
    }
    if is_chinese(text) {
        Lang::Zh
    } else {
        Lang::En
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_chinese_is_chinese() {
        assert!(is_chinese("知识就是力量"));
        assert!(is_chinese("你好"));
        assert!(is_chinese("今天天气不错，我们出去走走吧"));
    }

    #[test]
    fn chinese_starting_with_english_word_is_still_chinese() {
        // 首字判定会在这里翻车，必须看整段比例
        assert!(is_chinese("iPhone手机很好用"));
        assert!(is_chinese("GDP增长了5个百分点"));
        assert!(is_chinese("OK，那我们今天就开会"));
        assert!(is_chinese("Hello 这是一段用来验证检测逻辑的中文文本"));
    }

    #[test]
    fn chinese_with_a_few_english_terms_is_still_chinese() {
        assert!(is_chinese("那个 meeting 的议程已经定下来了"));
        assert!(is_chinese("请把 PR 提交到 main 分支上"));
    }

    #[test]
    fn digits_and_punctuation_do_not_dilute_the_ratio() {
        // 「5%」「2024」这些在任何语言里长得都一样，不该把判定往阈值上拉
        assert!(is_chinese("2024年GDP增长5%"));
        assert!(is_chinese("2024年GDP增长了5%，比去年高2个百分点"));
    }

    #[test]
    fn text_without_han_characters_is_not_chinese() {
        // 没有汉字就谈不上是中文。这类文本本来也没法翻译，译文方向无所谓
        assert!(!is_chinese("——……「」《》"));
        assert!(!is_chinese("?!@#$%"));
    }

    #[test]
    fn non_chinese_texts() {
        assert!(!is_chinese("hello world"));
        assert!(!is_chinese("The quick brown fox jumps over the lazy dog"));
        assert!(!is_chinese("The 会议 will be held tomorrow"));
        assert!(!is_chinese("12345"));
        assert!(!is_chinese(""));
        assert!(!is_chinese("AI"));
    }

    #[test]
    fn japanese_and_korean_are_not_chinese() {
        // 假名/谚文是专属字符，出现即非中文，否则日译中会被错译成日译英
        assert!(!is_chinese("日本語を勉強しています"));
        assert!(!is_chinese("こんにちは、元気ですか"));
        assert!(!is_chinese("안녕하세요"));
    }

    #[test]
    fn auto_target_flips_between_zh_and_en() {
        assert_eq!(auto_target("知识就是力量"), Lang::En);
        assert_eq!(auto_target("iPhone手机很好用"), Lang::En);
        assert_eq!(auto_target("hello world"), Lang::Zh);
        assert_eq!(auto_target("le temps est beau"), Lang::Zh);
        assert_eq!(auto_target("日本語を勉強しています"), Lang::Zh);
    }

    #[test]
    fn display_fallback_names_the_script() {
        assert_eq!(detect("こんにちは"), Lang::Ja);
        assert_eq!(detect("안녕하세요"), Lang::Ko);
        assert_eq!(detect("你好"), Lang::Zh);
        assert_eq!(detect("hello"), Lang::En);
    }
}
