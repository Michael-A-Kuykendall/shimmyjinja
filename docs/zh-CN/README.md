# shimmyjinja — 纯 Rust 聊天模板渲染器

**语言：** [English](../../README.md) · **简体中文** · [繁體中文](../zh-TW/README.md)

**当前版本：0.5.0。** shimmyjinja 用 Rust 解释 GGUF / Hugging Face `chat_template` 中使用的特定 Jinja 风格语法，不依赖 Python。Shimmy 和 Airframe 使用它将聊天消息格式化为模型提示。它并非完整 Jinja2 实现，也不执行 GPU 推理。

## 使用方法

```toml
[dependencies]
shimmyjinja = "0.5.0"
```

推荐使用返回 `Result` 的非 panic API：`try_render_chat_template` 或 `try_render_chat_template_with_context`。对模板错误进行显式处理，不要假定任意 Jinja2 扩展都被支持。可用语法和接口以[英文 README](../../README.md)及[公开 Rust API](https://docs.rs/shimmyjinja)为准。

## 稳定性与许可

0.5.0 将内部 `ast`、`eval`、`lexer`、`parser` 模块改为 `pub(crate)`；下游不应将这些内部模块视作稳定接口。参阅[变更记录](../../CHANGELOG.md)。许可证为 **MIT**，参阅[LICENSE](../../LICENSE)。

## 相关项目

[Shimmy](https://github.com/Michael-A-Kuykendall/shimmy) · [Airframe](https://github.com/Michael-A-Kuykendall/airframe) · [英文 README](../../README.md)。

英文 API 和版本记录为权威依据；本翻译不保证完整 Jinja2 兼容性。