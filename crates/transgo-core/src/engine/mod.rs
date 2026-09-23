//! 翻译引擎抽象与注册表。

mod aliyun;
mod azure;
mod baidu;
mod baidu_llm;
mod deepl;
mod google;
mod mymemory;
mod openai;
mod tencent;
mod util;
mod volcano;
mod youdao;

use std::sync::Arc;

use async_trait::async_trait;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::lang::Lang;
use crate::types::{Request, Translation};

/// 引擎优先级：越靠前越优先被选为默认引擎。
///
/// 排序依据是「国内可直连 + 免费额度」。`mymemory` 免注册，排在末尾。
pub const ORDER: &[&str] = &[
    "deepl", "tencent", "volcano", "aliyun", "baidu", "baidu-llm", "azure", "google", "youdao",
    "llm", "mymemory",
];

/// 一个翻译后端。
#[async_trait]
pub trait Engine: Send + Sync {
    /// 稳定 id，用在 `-e` 与配置文件里
    fn id(&self) -> &'static str;
    /// 展示名
    fn name(&self) -> &'static str;
    /// 免费额度说明
    fn quota(&self) -> &'static str;
    /// 申请地址
    fn signup(&self) -> &'static str;
    /// API Key 是否已就位
    fn configured(&self) -> bool;
    /// 支持的语种；`None` = 不做限制（LLM）
    fn languages(&self) -> Option<&'static [Lang]>;

    /// 该引擎能否处理这条请求
    fn supports(&self, req: &Request) -> bool {
        match self.languages() {
            None => true,
            Some(list) => list.contains(&req.to) && req.from.map_or(true, |f| f == Lang::Auto || list.contains(&f)),
        }
    }

    /// 执行翻译。`req.from == None` 时由引擎自动检测源语种。
    async fn translate(&self, req: &Request) -> Result<Translation>;
}

/// 按 [`ORDER`] 构造全部引擎。未配置的也照样返回，由调用方决定是否跳过。
pub fn build_all(cfg: &Config) -> Vec<Arc<dyn Engine>> {
    let timeout = crate::timeout_from(cfg);
    let client = crate::http_client(timeout).unwrap_or_default();
    ORDER
        .iter()
        .map(|id| -> Arc<dyn Engine> {
            match *id {
                "deepl" => Arc::new(deepl::DeepL::new(client.clone(), &cfg.deepl)),
                "tencent" => Arc::new(tencent::Tencent::new(client.clone(), &cfg.tencent)),
                "volcano" => Arc::new(volcano::Volcano::new(client.clone(), &cfg.volcano)),
                "aliyun" => Arc::new(aliyun::Aliyun::new(client.clone(), &cfg.aliyun)),
                "baidu" => Arc::new(baidu::Baidu::new(client.clone(), &cfg.baidu)),
                "baidu-llm" => Arc::new(baidu_llm::BaiduLlm::new(client.clone(), &cfg.baidu)),
                "azure" => Arc::new(azure::Azure::new(client.clone(), &cfg.azure)),
                "google" => Arc::new(google::Google::new(client.clone(), &cfg.google)),
                "youdao" => Arc::new(youdao::Youdao::new(client.clone(), &cfg.youdao)),
                "llm" => Arc::new(openai::Llm::new(client.clone(), &cfg.llm)),
                "mymemory" => Arc::new(mymemory::MyMemory::new(client.clone(), &cfg.mymemory)),
                other => unreachable!("ORDER 里出现未实现的引擎 {other}"),
            }
        })
        .collect()
}

/// 按 id 取引擎
pub fn lookup(cfg: &Config, id: &str) -> Result<Arc<dyn Engine>> {
    build_all(cfg)
        .into_iter()
        .find(|e| e.id().eq_ignore_ascii_case(id))
        .ok_or_else(|| Error::UnknownEngine(id.to_string()))
}

/// 决定默认引擎：
/// 1. 配置里 `default.engine` 指定了就用它
/// 2. 否则取 [`ORDER`] 里第一个 `configured()` 的
/// 3. 都没配置则退回免注册的 `mymemory`
pub fn pick_default(cfg: &Config, req: &Request) -> Result<Arc<dyn Engine>> {
    let all = build_all(cfg);

    if let Some(want) = cfg.default.engine.as_deref().filter(|s| !s.is_empty()) {
        return all
            .into_iter()
            .find(|e| e.id().eq_ignore_ascii_case(want))
            .ok_or_else(|| Error::UnknownEngine(want.to_string()));
    }

    if let Some(e) = all.iter().find(|e| e.configured() && e.supports(req)) {
        return Ok(Arc::clone(e));
    }

    // 没有任何已配置引擎支持这个语种对，退回第一个支持它的
    if let Some(e) = all.iter().find(|e| e.supports(req)) {
        return Ok(Arc::clone(e));
    }

    Err(Error::UnsupportedPair {
        engine: "任意",
        from: req.from.map(|l| l.name_zh().to_string()).unwrap_or_else(|| "自动".into()),
        to: req.to.name_zh().to_string(),
    })
}
