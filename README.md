# shimmyjinja

[![crates.io](https://img.shields.io/crates/v/shimmyjinja.svg)](https://crates.io/crates/shimmyjinja)
[![docs.rs](https://docs.rs/shimmyjinja/badge.svg)](https://docs.rs/shimmyjinja)
[![CI](https://github.com/Michael-A-Kuykendall/shimmyjinja/actions/workflows/ci.yml/badge.svg)](https://github.com/Michael-A-Kuykendall/shimmyjinja/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Pure-Rust Jinja2 engine for Hugging Face `chat_template` strings — the layer
that turns a raw GGUF file's template field into a correctly-formatted LLM prompt,
no Python required.

Part of the **shimmy** inference ecosystem:

| Crate | Role |
|---|---|
| **shimmyjinja** ← you are here | Jinja2 template engine for `chat_template` strings |
| [**shimmytok**](https://crates.io/crates/shimmytok) | GGUF-native tokenizer (BPE/SentencePiece) |
| [**airframe**](https://github.com/Michael-A-Kuykendall/airframe) | WebGPU inference server — uses both shimmytok and shimmyjinja |
| [**shimmy**](https://github.com/Michael-A-Kuykendall/shimmy) | OpenAI-compatible server powered by Airframe |

---

## What it does

Every GGUF model file ships a `tokenizer.chat_template` key — a Jinja2 template
string that controls how a list of chat messages gets formatted into the single
prompt string the model expects. Example (TinyLlama):

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

`shimmyjinja` evaluates templates like this in pure Rust. No Python process, no
`jinja2` dependency, no subprocess call to HuggingFace `transformers`.

---

## Supported Jinja2 subset

Everything used by real production `chat_template` strings today:

| Feature | Example |
|---|---|
| `for` loops | `{% for message in messages %}...{% endfor %}` |
| `if` / `elif` / `else` | `{% if message['role'] == 'user' %}` |
| String concatenation | `'<s>' + message['content']` |
| Equality / comparison | `==`, `!=`, `<`, `>`, `<=`, `>=` |
| Boolean logic | `and`, `or`, `not` |
| Membership test | `in`, `not in` |
| Inline ternary | `'yes' if flag else 'no'` |
| `namespace()` | `{% set ns = namespace(found=false) %}` |
| `set` / dotted `set` | `{% set ns.found = true %}` |
| `raise_exception()` | Raises on invalid usage |
| Method calls | `message.get('content', '')` |
| Context variables | `bos_token`, `eos_token`, `add_generation_prompt` |
| Bracket access | `message['role']` |

### Supported model families (tested with real GGUF files)

TinyLlama · Llama 3.2 · Mistral · Gemma 2 · Phi-3 / Phi-3.5 · Qwen 2 · Qwen 3 · DeepSeek-LLM

---

## Quick start

```toml
[dependencies]
shimmyjinja = "0.4"
```

```rust
use shimmyjinja::{ChatMessage, RenderContext, render_chat_template_with_context};

let template = r#"{% for message in messages %}{{'<|im_start|>' + message['role'] + '\n' + message['content'] + '<|im_end|>' + '\n'}}{% endfor %}{% if add_generation_prompt %}{{'<|im_start|>assistant\n'}}{% endif %}"#;

let messages = vec![
    ChatMessage { role: "user".into(), content: "Hello!".into() },
];

let mut ctx = RenderContext::new();
ctx.set_var("bos_token", "<s>");
ctx.set_var("eos_token", "</s>");
ctx.set_flag("add_generation_prompt", true);

let prompt = render_chat_template_with_context(template, &messages, &ctx);
// "<|im_start|>user\nHello!<|im_end|>\n<|im_start|>assistant\n"
```

### Using with a GGUF file

Pair with [shimmytok](https://crates.io/crates/shimmytok) to extract both the
template and token strings directly from the model file:

```rust
use shimmytok::Tokenizer;

let tok = Tokenizer::from_gguf_file("model.gguf")?;
let template = tok.chat_template().unwrap();
let bos = tok.bos_token();
let eos = tok.eos_token();
// then render with shimmyjinja as above
```

---

## Testing

```bash
cargo test          # 81 tests: unit + integration + property-based
```

The test suite covers:

- **Unit tests** — lexer, parser, evaluator edge cases (12 tests)
- **Real model templates** — embedded verbatim `chat_template` strings from 6 model families, disk-free (21 tests)
- **GGUF extraction tests** — end-to-end with real GGUF files on disk, skip-if-missing (9 tests)
- **Property-based tests** — 13 proptest properties: determinism, no content loss, no unresolved tags, token literal pass-through, generation-prompt suppression, long content, empty input

---

## Design goals

- **Zero dependencies at runtime** — no `proc-macro`, no heavy crates. The `[dependencies]` section of `Cargo.toml` is empty.
- **`cargo publish` clean** — no `build.rs`, no C/C++ compilation, no bindgen.
- **Explicit newline semantics** — no newlines are invented by the engine; all whitespace comes from the template string after JSON decoding.
- **Fail loudly on bad templates** — `parse()` returns `Err` rather than silently producing wrong output.

---

## Governance & contributions

Maintainer: **Michael Kuykendall** (michaelallenkuykendall@gmail.com).

Significant behavioral changes affecting `chat_template` compatibility should
be discussed in an issue first. Small focused fixes welcome as PRs.

## License

MIT — see [LICENSE](LICENSE).
