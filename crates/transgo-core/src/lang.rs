//! 统一语种枚举，以及各家引擎语种代码的映射。
//!
//! 各家 API 的语种代码极不统一（百度日语是 `jp`、有道简中是 `zh-CHS`、火山用 BCP-47 `zh-Hant`），
//! 这里定义一份通用代码，每个引擎自己做映射。

use serde::{Deserialize, Deserializer, Serialize, Serializer};

macro_rules! langs {
    ($($v:ident => { code: $code:literal, zh: $zh:literal }),+ $(,)?) => {
        /// 语种。`Auto` 仅用于请求侧的「自动检测」。
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Lang {
            /// 自动检测
            Auto,
            $($v),+
        }

        impl Lang {
            /// 除 `Auto` 之外的全部语种
            pub const ALL: &'static [Lang] = &[$(Lang::$v),+];

            /// 通用语种代码（ISO 639-1 风格），各家映射以此为基准
            pub const fn code(self) -> &'static str {
                match self {
                    Lang::Auto => "auto",
                    $(Lang::$v => $code),+
                }
            }

            /// 中文名
            pub const fn name_zh(self) -> &'static str {
                match self {
                    Lang::Auto => "自动检测",
                    $(Lang::$v => $zh),+
                }
            }

            /// 英文名，用于写进 LLM 提示词
            pub const fn name_en(self) -> &'static str {
                match self {
                    Lang::Auto => "auto-detect",
                    $(Lang::$v => stringify!($v)),+
                }
            }
        }
    };
}

langs! {
    Zh   => { code: "zh",    zh: "简体中文" },
    ZhTw => { code: "zh-TW", zh: "繁体中文" },
    En   => { code: "en",    zh: "英语" },
    Ja   => { code: "ja",    zh: "日语" },
    Ko   => { code: "ko",    zh: "韩语" },
    Fr   => { code: "fr",    zh: "法语" },
    De   => { code: "de",    zh: "德语" },
    Es   => { code: "es",    zh: "西班牙语" },
    Pt   => { code: "pt",    zh: "葡萄牙语" },
    It   => { code: "it",    zh: "意大利语" },
    Ru   => { code: "ru",    zh: "俄语" },
    Ar   => { code: "ar",    zh: "阿拉伯语" },
    Th   => { code: "th",    zh: "泰语" },
    Vi   => { code: "vi",    zh: "越南语" },
    Id   => { code: "id",    zh: "印尼语" },
    Ms   => { code: "ms",    zh: "马来语" },
    Tr   => { code: "tr",    zh: "土耳其语" },
    Hi   => { code: "hi",    zh: "印地语" },
    Nl   => { code: "nl",    zh: "荷兰语" },
    Pl   => { code: "pl",    zh: "波兰语" },
    Sv   => { code: "sv",    zh: "瑞典语" },
    Da   => { code: "da",    zh: "丹麦语" },
    Fi   => { code: "fi",    zh: "芬兰语" },
    El   => { code: "el",    zh: "希腊语" },
    Cs   => { code: "cs",    zh: "捷克语" },
    Ro   => { code: "ro",    zh: "罗马尼亚语" },
    Uk   => { code: "uk",    zh: "乌克兰语" },
    Fa   => { code: "fa",    zh: "波斯语" },
    He   => { code: "he",    zh: "希伯来语" },
}

impl Lang {
    /// 从任意语种代码解析。大小写、`_`/`-` 不敏感，并接受各家常见别名。
    pub fn from_code(s: &str) -> Option<Lang> {
        let raw = s.trim();
        if raw.is_empty() {
            return Some(Lang::Auto);
        }
        let n = raw.to_ascii_lowercase().replace('_', "-");
        Some(match n.as_str() {
            "auto" | "autodetect" | "any" | "unknown" | "detect" => Lang::Auto,

            // 中文：各家代码差异最大
            "zh" | "zh-cn" | "zh-hans" | "zh-sg" | "chi" | "zho" | "chs" | "zh-chs" | "cn" | "chinese"
            | "zh-hans-cn" => Lang::Zh,
            "zh-tw" | "zh-hk" | "zh-mo" | "zh-hant" | "cht" | "zh-cht" | "zh-hant-tw" | "zh-hant-hk"
            | "traditional" => Lang::ZhTw,

            "en" | "en-us" | "en-gb" | "en-au" | "eng" | "english" => Lang::En,
            "ja" | "jp" | "jpn" | "japanese" => Lang::Ja,
            "ko" | "kr" | "kor" | "korean" => Lang::Ko,
            "fr" | "fra" | "fre" | "french" => Lang::Fr,
            "de" | "deu" | "ger" | "german" => Lang::De,
            "es" | "spa" | "spanish" => Lang::Es,
            "pt" | "pt-br" | "pt-pt" | "por" | "portuguese" => Lang::Pt,
            "it" | "ita" | "italian" => Lang::It,
            "ru" | "rus" | "russian" => Lang::Ru,
            "ar" | "ara" | "arabic" => Lang::Ar,
            "th" | "tha" | "thai" => Lang::Th,
            "vi" | "vie" | "vietnamese" => Lang::Vi,
            "id" | "ind" | "indonesian" => Lang::Id,
            "ms" | "may" | "msa" | "malay" => Lang::Ms,
            "tr" | "tur" | "turkish" => Lang::Tr,
            "hi" | "hin" | "hindi" => Lang::Hi,
            "nl" | "nld" | "dut" | "dutch" => Lang::Nl,
            "pl" | "pol" | "polish" => Lang::Pl,
            "sv" | "swe" | "swedish" => Lang::Sv,
            "da" | "dan" | "danish" => Lang::Da,
            "fi" | "fin" | "finnish" => Lang::Fi,
            "el" | "gre" | "ell" | "greek" => Lang::El,
            "cs" | "ces" | "cze" | "czech" => Lang::Cs,
            "ro" | "ron" | "rum" | "romanian" => Lang::Ro,
            "uk" | "ukr" | "ukrainian" => Lang::Uk,
            "fa" | "fas" | "per" | "persian" => Lang::Fa,
            "he" | "iw" | "heb" | "hebrew" => Lang::He,

            // 退回通用代码匹配
            _ => {
                return Lang::ALL
                    .iter()
                    .copied()
                    .find(|l| l.code().eq_ignore_ascii_case(raw))
            }
        })
    }
}

impl std::fmt::Display for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code())
    }
}

impl Serialize for Lang {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(self.code())
    }
}

impl<'de> Deserialize<'de> for Lang {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Lang::from_code(&raw).ok_or_else(|| serde::de::Error::custom(format!("未知语种代码 {raw}")))
    }
}
