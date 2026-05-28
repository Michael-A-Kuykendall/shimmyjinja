# shimmyjinja

[![crates.io](https://img.shields.io/crates/v/shimmyjinja.svg)](https://crates.io/crates/shimmyjinja)
[![docs.rs](https://docs.rs/shimmyjinja/badge.svg)](https://docs.rs/shimmyjinja)
[![CI](https://github.com/Michael-A-Kuykendall/shimmyjinja/actions/workflows/ci.yml/badge.svg)](https://github.com/Michael-A-Kuykendall/shimmyjinja/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

纯 Rust 实现的 Jinja2 引擎，专为 Hugging Face `chat_template` 字符串设计。  
无需 Python，无需 `transformers`，直接从 GGUF 文件读取模板并生成提示词。

本库是 **shimmy** 推理生态系统的一部分：

| 库 | 功能 |
|---|---|
| **shimmyjinja** ← 当前库 | `chat_template` 字符串的 Jinja2 模板引擎 |
| [**shimmytok**](https://crates.io/crates/shimmytok) | 原生读取 GGUF 格式的分词器（BPE/SentencePiece） |
| [**airframe**](https://github.com/Michael-A-Kuykendall/airframe) | WebGPU 推理服务器，同时使用 shimmytok 和 shimmyjinja |
| [**shimmy**](https://github.com/Michael-A-Kuykendall/shimmy) | 基于 Airframe 的 OpenAI 兼容推理服务 |

---

## 功能介绍

每个 GGUF 模型文件中都包含一个 `tokenizer.chat_template` 字段——这是一段 Jinja2 模板字符串，用于将对话消息列表格式化为模型所需的提示词。例如（TinyLlama）：

```jinja
{% for message in messages %}
{% if message['role'] == 'user' %}
<|user|>
{{ message['content'] }}{{ eos_token }}
{% endif %}
{% endfor %}
{% if add_generation_prompt %}<|assistant|>
{% endif %}
```

`shimmyjinja` 用纯 Rust 对此类模板进行求值。无需 Python 进程，无需 `jinja2` 依赖，无需调用 HuggingFace `transformers`。

---

## 支持的 Jinja2 特性

覆盖当前主流 `chat_template` 字符串中的所有用法：

| 特性 | 示例 |
|---|---|
| `for` 循环 | `{% for message in messages %}...{% endfor %}` |
| `if` / `elif` / `else` | `{% if message['role'] == 'user' %}` |
| 字符串拼接 | `'<s>' + message['content']` |
| 比较运算 | `==`、`!=`、`<`、`>`、`<=`、`>=` |
| 布尔逻辑 | `and`、`or`、`not` |
| 成员检测 | `in`、`not in` |
| 内联三元表达式 | `'yes' if flag else 'no'` |
| `namespace()` | `{% set ns = namespace(found=false) %}` |
| `set` / 点式 `set` | `{% set ns.found = true %}` |
| `raise_exception()` | 对无效用法抛出错误 |
| 方法调用 | `message.get('content', '')` |
| 上下文变量 | `bos_token`、`eos_token`、`add_generation_prompt` |
| 方括号访问 | `message['role']` |

### 已测试的模型系列（使用真实 GGUF 文件验证）

TinyLlama · Llama 3.2 · Mistral · Gemma 2 · Phi-3 / Phi-3.5 · Qwen 2 · Qwen 3 · DeepSeek-LLM

---

## 快速上手

```toml
[dependencies]
shimmyjinja = "0.4"
```

```rust
use shimmyjinja::{ChatMessage, RenderContext, render_chat_template_with_context};

let template = r#"{% for message in messages %}{{'<|im_start|>' + message['role'] + '\n' + message['content'] + '<|im_end|>' + '\n'}}{% endfor %}{% if add_generation_prompt %}{{'<|im_start|>assistant\n'}}{% endif %}"#;

let messages = vec![
    ChatMessage { role: "user".into(), content: "你好！".into() },
];

let mut ctx = RenderContext::new();
ctx.set_var("bos_token", "<s>");
ctx.set_var("eos_token", "</s>");
ctx.set_flag("add_generation_prompt", true);

let prompt = render_chat_template_with_context(template, &messages, &ctx);
// "<|im_start|>user\n你好！<|im_end|>\n<|im_start|>assistant\n"
```

### 与 GGUF 文件配合使用

搭配 [shimmytok](https://crates.io/crates/shimmytok) 直接从模型文件中提取模板和 token 字符串：

```rust
use shimmytok::Tokenizer;

let tok = Tokenizer::from_gguf_file("model.gguf")?;
let template = tok.chat_template().unwrap();
let bos = tok.bos_token();
let eos = tok.eos_token();
// 然后使用 shimmyjinja 进行渲染（如上所示）
```

---

## 测试

```bash
cargo test   # 81 个测试：单元测试 + 集成测试 + 基于属性的测试
```

测试套件涵盖：

- **单元测试** — 词法分析器、解析器、求值器边界情况（12 个）
- **真实模型模板测试** — 内嵌来自 6 个模型系列的真实 `chat_template` 字符串，无需磁盘文件（21 个）
- **GGUF 提取测试** — 端对端测试，使用真实 GGUF 文件，文件不存在时自动跳过（9 个）
- **基于属性的测试** — 13 个 proptest 属性：确定性、无内容丢失、无未解析标签、token 字面量透传、生成提示词抑制、长内容、空输入

---

## 设计目标

- **运行时零依赖** — 无 `proc-macro`，无重量级 crate。`Cargo.toml` 的 `[dependencies]` 为空。
- **`cargo publish` 友好** — 无 `build.rs`，无 C/C++ 编译，无 bindgen。
- **显式换行语义** — 引擎不会凭空生成换行符；所有空白均来自 JSON 解码后的模板字符串。
- **模板错误快速失败** — `parse()` 返回 `Err` 而不是静默输出错误结果。

---

## 许可证

MIT — 详见 [LICENSE](LICENSE)。

---

[English README](README.md)
