//! `~/.config/transgo/config.toml` 的读写。
//!
//! 环境变量优先于配置文件，方便脚本和 CI 覆盖。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// 配置文件路径。可用 `TRANSGO_CONFIG` 覆盖。
pub fn config_path() -> PathBuf {
    if let Some(p) = std::env::var_os("TRANSGO_CONFIG") {
        return PathBuf::from(p);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("transgo")
        .join("config.toml")
}

/// 通用配置：只有一把 API Key 的服务商
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyCfg {
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Defaults {
    /// 默认引擎 id；留空则按 [`crate::engine::ORDER`] 取第一个已配置的
    pub engine: Option<String>,
    /// 默认目标语种，`auto` = 中英互译
    pub to: Option<String>,
    /// 默认源语种，`auto` = 自动检测
    pub from: Option<String>,
    /// 请求超时（秒）
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MyMemoryCfg {
    /// 留邮箱可把免费额度从 5 千字符/天提到 5 万字符/天
    pub email: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DeepLCfg {
    pub api_key: Option<String>,
    /// `free` 或 `pro`；留空则按 Key 是否以 `:fx` 结尾自动判断
    pub plan: Option<String>,
    /// 覆盖接口地址（走自建网关时用）
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AzureCfg {
    pub api_key: Option<String>,
    /// 多服务资源或区域资源需要
    pub region: Option<String>,
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TencentCfg {
    pub secret_id: Option<String>,
    pub secret_key: Option<String>,
    pub region: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BaiduCfg {
    pub app_id: Option<String>,
    pub secret: Option<String>,
    /// API Key 鉴权（Bearer），控制台「API Key 管理」创建。
    /// 填了它 `baidu-llm` 就免 MD5 签名；`baidu` 通用翻译仍走签名
    pub api_key: Option<String>,
    /// 翻译指令（仅 `baidu-llm` 生效），如「采用意译」，上限 500 字符。
    /// 每次调用可用 CLI 的 --instruction 覆盖
    pub reference: Option<String>,
    /// 术语库干预开关（仅 `baidu-llm` 生效），术语表在控制台「我的术语库」维护
    pub need_intervene: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AliyunCfg {
    pub access_key_id: Option<String>,
    pub access_key_secret: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VolcanoCfg {
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub region: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YoudaoCfg {
    pub app_key: Option<String>,
    pub app_secret: Option<String>,
}

/// 任意 OpenAI 兼容接口（OpenAI / DeepSeek / 硅基流动 / Ollama / …）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmCfg {
    pub api_key: Option<String>,
    /// 例如 `https://api.openai.com/v1`、`http://localhost:11434/v1`
    pub base_url: Option<String>,
    pub model: Option<String>,
    /// 覆盖默认的翻译系统提示词
    pub system_prompt: Option<String>,
    pub temperature: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GuiCfg {
    /// 字体缩放系数（0.5 ~ 3.0）。高分屏嫌字大、外接屏嫌字小的时候调它
    pub font_scale: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub default: Defaults,
    pub gui: GuiCfg,
    pub mymemory: MyMemoryCfg,
    pub deepl: DeepLCfg,
    pub google: KeyCfg,
    pub azure: AzureCfg,
    pub tencent: TencentCfg,
    pub baidu: BaiduCfg,
    pub aliyun: AliyunCfg,
    pub volcano: VolcanoCfg,
    pub youdao: YoudaoCfg,
    pub llm: LlmCfg,
}

impl Config {
    /// 只读配置文件（不存在则返回默认值），**不**叠加环境变量。
    ///
    /// 写回场景必须用这个，否则会把当前 shell 里的 `TRANSGO_*` 一并持久化进文件。
    pub fn load_file() -> Result<Self> {
        let path = config_path();
        if path.is_file() {
            let raw = std::fs::read_to_string(&path)?;
            toml::from_str(&raw).map_err(|e| Error::Config(format!("{} 解析失败: {e}", path.display())))
        } else {
            Ok(Config::default())
        }
    }

    /// 读取配置文件，再叠加环境变量。日常读取用这个。
    pub fn load() -> Result<Self> {
        let mut cfg = Self::load_file()?;
        cfg.apply_env();
        Ok(cfg)
    }

    /// 写回配置文件，自动建目录。
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let raw = toml::to_string_pretty(self)
            .map_err(|e| Error::Config(format!("序列化失败: {e}")))?;
        let banner = "# transgo 配置文件\n# 键的含义见 transgo config list，环境变量 TRANSGO_* 可覆盖同名项。\n\n";
        std::fs::write(path, format!("{banner}{raw}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // 里面有 API Key，收紧到 600
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// 环境变量覆盖。只覆盖有定义的项，未定义的保持配置文件里的值。
    fn apply_env(&mut self) {
        macro_rules! overlay {
            ($field:expr, $var:literal) => {
                if let Ok(v) = std::env::var($var) {
                    if !v.trim().is_empty() {
                        $field = Some(v);
                    }
                }
            };
        }
        overlay!(self.default.engine, "TRANSGO_ENGINE");
        overlay!(self.default.to, "TRANSGO_TO");
        overlay!(self.default.from, "TRANSGO_FROM");
        if let Ok(v) = std::env::var("TRANSGO_GUI_FONT_SCALE") {
            if let Ok(n) = v.trim().parse::<f64>() {
                self.gui.font_scale = Some(n);
            }
        }
        overlay!(self.mymemory.email, "TRANSGO_MYMEMORY_EMAIL");
        overlay!(self.deepl.api_key, "TRANSGO_DEEPL_API_KEY");
        overlay!(self.deepl.plan, "TRANSGO_DEEPL_PLAN");
        overlay!(self.google.api_key, "TRANSGO_GOOGLE_API_KEY");
        overlay!(self.azure.api_key, "TRANSGO_AZURE_API_KEY");
        overlay!(self.azure.region, "TRANSGO_AZURE_REGION");
        overlay!(self.tencent.secret_id, "TRANSGO_TENCENT_SECRET_ID");
        overlay!(self.tencent.secret_key, "TRANSGO_TENCENT_SECRET_KEY");
        overlay!(self.tencent.region, "TRANSGO_TENCENT_REGION");
        overlay!(self.baidu.app_id, "TRANSGO_BAIDU_APP_ID");
        overlay!(self.baidu.secret, "TRANSGO_BAIDU_SECRET");
        overlay!(self.baidu.api_key, "TRANSGO_BAIDU_API_KEY");
        overlay!(self.baidu.reference, "TRANSGO_BAIDU_REFERENCE");
        if let Ok(v) = std::env::var("TRANSGO_BAIDU_NEED_INTERVENE") {
            match v.trim() {
                "1" | "true" => self.baidu.need_intervene = Some(true),
                "0" | "false" => self.baidu.need_intervene = Some(false),
                _ => {}
            }
        }
        overlay!(self.aliyun.access_key_id, "TRANSGO_ALIYUN_ACCESS_KEY_ID");
        overlay!(self.aliyun.access_key_secret, "TRANSGO_ALIYUN_ACCESS_KEY_SECRET");
        overlay!(self.volcano.access_key_id, "TRANSGO_VOLCANO_ACCESS_KEY_ID");
        overlay!(self.volcano.secret_access_key, "TRANSGO_VOLCANO_SECRET_ACCESS_KEY");
        overlay!(self.volcano.region, "TRANSGO_VOLCANO_REGION");
        overlay!(self.youdao.app_key, "TRANSGO_YOUDAO_APP_KEY");
        overlay!(self.youdao.app_secret, "TRANSGO_YOUDAO_APP_SECRET");
        overlay!(self.llm.api_key, "TRANSGO_LLM_API_KEY");
        overlay!(self.llm.base_url, "TRANSGO_LLM_BASE_URL");
        overlay!(self.llm.model, "TRANSGO_LLM_MODEL");
    }

    /// 以点分路径读取，如 `deepl.api_key`。返回 `None` 表示未设置。
    pub fn get(&self, dotted: &str) -> Result<Option<String>> {
        let doc = self.to_value()?;
        let val = doc
            .pointer(&to_pointer(dotted))
            .ok_or_else(|| Error::Config(format!("没有配置项「{dotted}」")))?;
        Ok(match val {
            serde_json::Value::Null => None,
            serde_json::Value::String(s) => Some(s.clone()),
            other => Some(other.to_string()),
        })
    }

    /// 以点分路径写入。值为空串表示清除。
    pub fn set(&mut self, dotted: &str, value: &str) -> Result<()> {
        let leaf = if value.is_empty() {
            serde_json::Value::Null
        } else {
            parse_scalar(value)
        };
        // parse_scalar 是对称的：纯数字、true/false 会按语义解析，此时没有退回余地；
        // 否则写入失败后还会按原样字符串再试一次（纯数字的百度 APP ID 走这条路）
        let no_fallback = leaf == serde_json::Value::String(value.to_string());
        match self.with_leaf(dotted, leaf) {
            Ok(next) => {
                *self = next;
                Ok(())
            }
            Err(first) if no_fallback => Err(first),
            Err(_) => {
                *self = self.with_leaf(dotted, serde_json::Value::String(value.to_string()))?;
                Ok(())
            }
        }
    }

    /// 返回一份写入了叶子值的新配置，不改动自身
    fn with_leaf(&self, dotted: &str, leaf: serde_json::Value) -> Result<Config> {
        let mut doc = self.to_value()?;
        {
            let mut cursor = &mut doc;
            let parts: Vec<&str> = dotted.split('.').filter(|p| !p.is_empty()).collect();
            if parts.is_empty() {
                return Err(Error::Config("配置项路径不能为空".into()));
            }
            let (last, parents) = parts
                .split_last()
                .expect("parts 已确认非空");
            for p in parents {
                cursor = cursor
                    .get_mut(*p)
                    .ok_or_else(|| Error::Config(format!("没有配置项「{dotted}」")))?;
            }
            if cursor.get(*last).is_none() && !cursor.is_object() {
                return Err(Error::Config(format!("没有配置项「{dotted}」")));
            }
            if !cursor.is_object() {
                return Err(Error::Config(format!("没有配置项「{dotted}」")));
            }
            cursor
                .as_object_mut()
                .expect("已确认是对象")
                .insert((*last).to_string(), leaf);
        }
        serde_json::from_value(doc)
            .map_err(|e| Error::Config(format!("写入「{dotted}」失败: {e}")))
    }

    /// 列出全部点分路径（含未设置的），用于 `transgo config list`。
    pub fn keys(&self) -> Vec<String> {
        fn walk(prefix: &str, v: &serde_json::Value, out: &mut Vec<String>) {
            match v {
                serde_json::Value::Object(map) => {
                    for (k, sub) in map {
                        let path = if prefix.is_empty() {
                            k.clone()
                        } else {
                            format!("{prefix}.{k}")
                        };
                        walk(&path, sub, out);
                    }
                }
                leaf => out.push(format!("{prefix} = {}", leaf_display(leaf))),
            }
        }
        let mut out = Vec::new();
        let doc = self.to_value().unwrap_or(serde_json::Value::Null);
        walk("", &doc, &mut out);
        out.sort();
        out
    }

    fn to_value(&self) -> Result<serde_json::Value> {
        serde_json::to_value(self).map_err(|e| Error::Config(format!("序列化失败: {e}")))
    }
}

fn leaf_display(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "(未设置)".to_string(),
        serde_json::Value::String(s) if s.is_empty() => "(未设置)".to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn to_pointer(dotted: &str) -> String {
    let mut p = String::from("");
    for part in dotted.split('.').filter(|p| !p.is_empty()) {
        p.push('/');
        p.push_str(&part.replace('~', "~0").replace('/', "~1"));
    }
    p
}

/// 把命令行传入的字符串按语义解析成标量：`true`/`false` → 布尔，纯数字 → 数值，其余 → 字符串
fn parse_scalar(s: &str) -> serde_json::Value {
    match s {
        "true" => return serde_json::Value::Bool(true),
        "false" => return serde_json::Value::Bool(false),
        "null" => return serde_json::Value::Null,
        _ => {}
    }
    if let Ok(n) = s.parse::<i64>() {
        return serde_json::Number::from(n).into();
    }
    if let Ok(n) = s.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(n) {
            return num.into();
        }
    }
    serde_json::Value::String(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_roundtrip() {
        let mut c = Config::default();
        c.set("deepl.api_key", "abc:fx").unwrap();
        assert_eq!(c.get("deepl.api_key").unwrap().as_deref(), Some("abc:fx"));

        c.set("deepl.api_key", "").unwrap();
        assert_eq!(c.get("deepl.api_key").unwrap(), None);
    }

    #[test]
    fn set_numeric_value_falls_back_to_string_field() {
        let mut c = Config::default();
        // 纯数字（百度 APP ID 这类）进字符串字段：按语义解析失败后退回原样字符串
        c.set("baidu.app_id", "12345678901234567").unwrap();
        assert_eq!(c.get("baidu.app_id").unwrap().as_deref(), Some("12345678901234567"));
        // 数字仍能进数值字段
        c.set("default.timeout_secs", "15").unwrap();
        assert_eq!(c.get("default.timeout_secs").unwrap().as_deref(), Some("15"));
        // 进不了数值字段的仍如实报错
        assert!(c.set("default.timeout_secs", "abc").is_err());
    }

    #[test]
    fn gui_font_scale_roundtrip() {
        let mut c = Config::default();
        c.set("gui.font_scale", "0.85").unwrap();
        assert_eq!(c.get("gui.font_scale").unwrap().as_deref(), Some("0.85"));
        assert!(c.set("gui.font_scale", "abc").is_err());
    }

    #[test]
    fn set_rejects_unknown_path() {
        let mut c = Config::default();
        assert!(c.set("nosuch.key", "x").is_err());
    }

    #[test]
    fn scalar_parsing() {
        assert_eq!(parse_scalar("true"), serde_json::Value::Bool(true));
        assert_eq!(parse_scalar("42"), serde_json::json!(42));
        assert_eq!(parse_scalar("abc"), serde_json::json!("abc"));
    }
}
