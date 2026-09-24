# transgo 待办（本地文件，不提交）

## 新登记

- [x] 英文文档：`README_EN.md`、`docs_en/`（已完成；中文版为主，不一致时以中文为准）
- [ ] 接入 Anthropic（Messages API）
      非 OpenAI 兼容格式：`/v1/messages`、`x-api-key` + `anthropic-version` 头，宜做独立引擎；
      无免费额度（按量计费），文档如实标注即可
- [ ] 接入 OpenAI Response API（`/v1/responses`）
      形态待定：`llm` 引擎加 `api_style` 开关，或独立引擎

## 路线图存量

- [ ] 划词翻译（模拟 Ctrl+C + 读剪贴板，需装 `wtype`）
- [ ] 截图 OCR 翻译（`grim` + `slurp` + `tesseract`）
- [ ] 生词本 / 历史记录

## 等外部条件

- [ ] Bearer 鉴权实测（等创建 API Key，5 分钟闭环）
- [ ] 火山 V4 签名探测清理（等火山 Key，用户暂缓）
- [ ] 其余引擎错误码实测（等各家 Key）
