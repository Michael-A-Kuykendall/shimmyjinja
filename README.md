**Languages / 语言 / 語言:** English · [简体中文](docs/zh-CN/README.md) · [繁體中文](docs/zh-TW/README.md)

# shimmyjinja

Current published version: **0.5.0**. This is a Jinja-like subset, not full Jinja2 compatibility or model inference certification. Use the fallible `try_render_chat_template` / `try_render_chat_template_with_context` APIs for untrusted templates; legacy render wrappers panic on errors. Internal `ast`, `eval`, `lexer` and `parser` modules are not public APIs.

[![crates.io](https://img.shields.io/crates/v/shimmyjinja.svg)](https://crates.io/crates/shimmyjinja) [![Trans rights](https://pride-badges.pony.workers.dev/static/v1?label=trans%20rights&stripeWidth=6&stripeColors=5BCEFA,F5A9B8,FFFFFF,F5A9B8,5BCEFA)](https://translifeline.org/) [![LGBTQ+ friendly](https://pride-badges.pony.workers.dev/static/v1?label=lgbtq%2B%20friendly&stripeWidth=6&stripeColors=E40303,FF8C00,FFED00,008026,24408E,732982)](https://www.thetrevorproject.org/)
[![docs.rs](https://docs.rs/shimmyjinja/badge.svg)](https://docs.rs/shimmyjinja)
[![CI](https://github.com/Michael-A-Kuykendall/shimmyjinja/actions/workflows/ci.yml/badge.svg)](https://github.com/Michael-A-Kuykendall/shimmyjinja/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Pure-Rust Jinja-like subset renderer for Hugging Face `chat_template` strings — the layer
that turns a raw GGUF file's template field into a correctly-formatted LLM prompt,
no Python required.

Part of the **shimmy** inference ecosystem:

| Crate | Role |
|---|---|
| **shimmyjinja** ← you are here | Jinja-style subset renderer for `chat_template` strings |
| [**shimmytok**](https://crates.io/crates/shimmytok) | GGUF-native tokenizer (BPE/SentencePiece) |
| [**airframe**](https://github.com/Michael-A-Kuykendall/airframe) | WebGPU inference engine — uses both shimmytok and shimmyjinja |
| [**shimmy**](https://github.com/Michael-A-Kuykendall/shimmy) | OpenAI-compatible server powered by Airframe |

---

### Support

This project is independently maintained and free forever. Sponsorship helps fund ongoing development, certification, compatibility work, and releases.

[Sponsor on GitHub](https://github.com/sponsors/Michael-A-Kuykendall) · [Buy Me a Coffee](https://www.buymeacoffee.com/michaelakuykendall)

Any amount is appreciated. Suggested tiers:

- **$5/month** — Thanks
- **$15/month** — Supporter
- **$50/month** — Backer
- **Custom** — [Email for invoicing](mailto:michaelallenkuykendall@gmail.com)

---

## What it does

Some GGUF model files provide a `tokenizer.chat_template` key — a Jinja2 template
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

Implemented syntax includes the following; unsupported templates must be handled as errors:

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
| `raise_exception()` | Currently a no-op; callers must validate role/template constraints themselves |
| Method calls | `message.get('content', '')` |
| Context variables | `bos_token`, `eos_token`, `add_generation_prompt` |
| Bracket access | `message['role']` |

### Template fixtures (not inference certification)

TinyLlama · Llama 3.2 · Mistral · Gemma 2 · Phi-3 / Phi-3.5 · Qwen 2 · Qwen 3 · DeepSeek-LLM

---

## Quick start

```toml
[dependencies]
shimmyjinja = "0.5.0"
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
cargo test          # integration, property-based and documentation tests
```

The test suite covers:

- **Edge-case tests** — lexer, parser and evaluator behavior through the public rendering API
- **Real model templates** — embedded verbatim `chat_template` strings from 6 model families, disk-free (21 tests)
- **GGUF extraction tests** — end-to-end with real GGUF files on disk, skip-if-missing (9 tests)
- **Property-based tests** — 13 proptest properties: determinism, no content loss, no unresolved tags, token literal pass-through, generation-prompt suppression, long content, empty input

---

## Design goals

- **Zero dependencies at runtime** — no `proc-macro`, no heavy crates. The `[dependencies]` section of `Cargo.toml` is empty.
- **`cargo publish` clean** — no `build.rs`, no C/C++ compilation, no bindgen.
- **Explicit newline semantics** — no newlines are invented by the engine; all whitespace comes from the template string after JSON decoding.
- **Fallible rendering** — `try_render_chat_template_with_context` returns `Err` on template failures. The internal parser is not a public API.

---

## Governance & contributions

Maintainer: **Michael Kuykendall** (michaelallenkuykendall@gmail.com).

This project is open source, not open contribution. Prospective co-maintainers should email the maintainer and obtain approval before submitting contributions.

## License

MIT — see [LICENSE](LICENSE).

## Support

This project is a safe space. Trans rights are human rights.

If you or someone you love needs support:

- [The Trevor Project](https://www.thetrevorproject.org/) — 24/7 for LGBTQ+ young people. Call 1-866-488-7386 or text START to 678-678
- [Trans Lifeline](https://translifeline.org/) — peer support run by and for trans people. US: 877-565-8860
- [988 Suicide & Crisis Lifeline](https://988lifeline.org/) — call or text 988
