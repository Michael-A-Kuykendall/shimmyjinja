# shimmyjinja — 純 Rust 聊天範本轉譯器

**語言：** [English](../../README.md) · [简体中文](../zh-CN/README.md) · **繁體中文**

**目前版本：0.5.0。** shimmyjinja 以 Rust 解析 GGUF／Hugging Face `chat_template` 所用的特定 Jinja 類似語法，不需要 Python。Shimmy 與 Airframe 使用它將聊天訊息格式化為模型提示。它不是完整 Jinja2 引擎，亦不進行 GPU 推論。

## 使用方式

```toml
[dependencies]
shimmyjinja = "0.5.0"
```

建議使用回傳 `Result` 的非 panic API：`try_render_chat_template` 或 `try_render_chat_template_with_context`。必須明確處理範本錯誤；不應假定任意 Jinja2 擴充功能均受支援。實際語法及介面請參閱[英文 README](../../README.md)與[公開 Rust API](https://docs.rs/shimmyjinja)。

## 穩定性及授權

0.5.0 將內部 `ast`、`eval`、`lexer`、`parser` 模組改為 `pub(crate)`；下游不可將這些內部模組視為穩定 API。參閱[變更記錄](../../CHANGELOG.md)。授權為 **MIT**，請參閱[LICENSE](../../LICENSE)。

## 相關專案

[Shimmy](https://github.com/Michael-A-Kuykendall/shimmy) · [Airframe](https://github.com/Michael-A-Kuykendall/airframe) · [英文 README](../../README.md)。

英文 API 與版本記錄為權威依據；本翻譯不承諾完整 Jinja2 相容性。