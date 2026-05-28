//! Real-model template tests.
//!
//! Every test here embeds the *verbatim* `tokenizer.chat_template` string that
//! was extracted from a real GGUF file, then feeds a concrete set of messages
//! through the renderer and asserts on the exact (or key-substring) output.
//!
//! Because the template strings are embedded as constants, these tests require
//! **no model files on disk** — they are pure math checks on the Jinja2
//! evaluation engine.
//!
//! # Supported model families (GPU inference path)
//!
//! | Arch string   | `ModelArch` variant | Examples                                    |
//! |---------------|---------------------|---------------------------------------------|
//! | `llama`       | `Llama`             | Llama 2/3/3.1/3.2/3.3, TinyLlama, Mistral, DeepSeek-LLM |
//! | `mistral`     | `Mistral`           | Mistral 0.1/0.2/0.3, Mixtral                |
//! | `phi` / `phi2` / `phi3` | `Phi`   | Phi-1 through Phi-3.5                       |
//! | `gemma`       | `Gemma`             | Gemma 1.x                                   |
//! | `gemma2`      | `Other("gemma2")`   | Gemma 2 (template works; tensor path falls  |
//! |               |                     |   back to `blk` prefix via `Other`)         |
//! | `qwen2`       | `Qwen2`             | Qwen 2 / Qwen 2.5 (all sizes)               |
//! | `qwen3`       | `Qwen3`             | Qwen 3 (0.6B – 235B)                        |
//!
//! # Roadmap candidates (arch recognised but no custom GPU kernel yet)
//!
//! `falcon`, `internlm2`, `cohere` (Command-R), `deepseek` (V3/R1),
//! `gptneox` / `stablelm`, `mamba`, `rwkv`, `starcoder2`, `llama4` (MoE).
//!
//! # GGUF header keys consumed by the template pipeline
//!
//! Key consumed in `make_prompt_renderer()` inside `shimmy_server_gpu.rs`:
//!   - `tokenizer.chat_template`  → Jinja2 template string
//!   - `tokenizer.ggml.bos_token_id` → u32; passed to
//!       `tokenizer.token_to_piece(id)` → bos_token string
//!   - `tokenizer.ggml.eos_token_id` → u32; same → eos_token string
//!
//! All other architectural keys (`{arch}.embedding_length`, `block_count`,
//! `attention.head_count`, …) are consumed by `spec.rs` / `BindlessMetadata`
//! for tensor layout and are unrelated to the prompt template.

use shimmyjinja::{render_chat_template_with_context, ChatMessage, RenderContext};

// ── Embedded template constants ───────────────────────────────────────────
//
// Extracted verbatim from real GGUF files.  Multi-line templates are stored
// with literal newlines inside the raw-string delimiters.

/// TinyLlama-1.1B-Chat-v1.0.Q4_0.gguf  (arch=llama, 410 chars)
///
/// Features exercised: `eos_token` concatenation, `loop.last`,
///   `add_generation_prompt` flag, multi-role dispatch.
const TMPL_TINYLLAMA: &str = concat!(
    "{% for message in messages %}\n",
    "{% if message['role'] == 'user' %}\n",
    "{{ '<|user|>\\n' + message['content'] + eos_token }}\n",
    "{% elif message['role'] == 'system' %}\n",
    "{{ '<|system|>\\n' + message['content'] + eos_token }}\n",
    "{% elif message['role'] == 'assistant' %}\n",
    "{{ '<|assistant|>\\n'  + message['content'] + eos_token }}\n",
    "{% endif %}\n",
    "{% if loop.last and add_generation_prompt %}\n",
    "{{ '<|assistant|>' }}\n",
    "{% endif %}\n",
    "{% endfor %}"
);

/// gemma-2-2b-it-Q4_K_M.gguf  (arch=gemma2, 591 chars)
///
/// Features: `bos_token`, `raise_exception` (no-op), `loop.index0 % 2`,
///   `message['role'] == 'assistant'` → rename to `model`, `|trim` filter,
///   string concatenation with explicit delimiters.
const TMPL_GEMMA2: &str = concat!(
    "{{ bos_token }}",
    "{% if messages[0]['role'] == 'system' %}",
    "{{ raise_exception('System role not supported') }}",
    "{% endif %}",
    "{% for message in messages %}",
    "{% if (message['role'] == 'user') != (loop.index0 % 2 == 0) %}",
    "{{ raise_exception('Conversation roles must alternate user/assistant/user/assistant/...') }}",
    "{% endif %}",
    "{% if (message['role'] == 'assistant') %}",
    "{% set role = 'model' %}",
    "{% else %}",
    "{% set role = message['role'] %}",
    "{% endif %}",
    "{{ '<start_of_turn>' + role + '\\n' + message['content'] | trim + '<end_of_turn>\\n' }}",
    "{% endfor %}",
    "{% if add_generation_prompt %}{{'<start_of_turn>model\\n'}}{% endif %}"
);

/// Phi-3.5-mini-instruct.Q4_K_M.gguf  (arch=phi3, 430 chars)
///
/// Features: `message['content']` as truthy guard, `add_generation_prompt`
///   path vs `eos_token` fallback, three-role dispatch.
const TMPL_PHI35: &str = concat!(
    "{% for message in messages %}",
    "{% if message['role'] == 'system' and message['content'] %}",
    "{{'<|system|>\\n' + message['content'] + '<|end|>\\n'}}",
    "{% elif message['role'] == 'user' %}",
    "{{'<|user|>\\n' + message['content'] + '<|end|>\\n'}}",
    "{% elif message['role'] == 'assistant' %}",
    "{{'<|assistant|>\\n' + message['content'] + '<|end|>\\n'}}",
    "{% endif %}",
    "{% endfor %}",
    "{% if add_generation_prompt %}{{ '<|assistant|>\\n' }}{% else %}{{ eos_token }}{% endif %}"
);

/// phi3-mini-4k-instruct-q4.gguf  (arch=phi3, 269 chars)
///
/// Features: `bos_token` prefix, parenthesised conditions, user→assistant
///   header on the same turn (no explicit generation-prompt check).
const TMPL_PHI3MINI: &str = concat!(
    "{{ bos_token }}",
    "{% for message in messages %}",
    "{% if (message['role'] == 'user') %}",
    "{{'<|user|>' + '\\n' + message['content'] + '<|end|>' + '\\n' + '<|assistant|>' + '\\n'}}",
    "{% elif (message['role'] == 'assistant') %}",
    "{{message['content'] + '<|end|>' + '\\n'}}",
    "{% endif %}",
    "{% endfor %}"
);

/// qwen2-7b-instruct-q4_k_m.gguf  (arch=qwen2, 328 chars)
///
/// Features: `loop.first`, `messages[0]['role']` integer-indexed nested
///   lookup, implicit system injection when no system message present,
///   ChatML delimiter style.
const TMPL_QWEN2: &str = concat!(
    "{% for message in messages %}",
    "{% if loop.first and messages[0]['role'] != 'system' %}",
    "{{ '<|im_start|>system\\nYou are a helpful assistant.<|im_end|>\\n' }}",
    "{% endif %}",
    "{{'<|im_start|>' + message['role'] + '\\n' + message['content'] + '<|im_end|>' + '\\n'}}",
    "{% endfor %}",
    "{% if add_generation_prompt %}{{ '<|im_start|>assistant\\n' }}{% endif %}"
);

/// deepseek-llm-7b-chat.Q4_K_M.gguf  (arch=llama, 459 chars)
///
/// Features: `not X is defined` → define-guard pattern, `bos_token` prefix,
///   plain-text delimiters (`User:` / `Assistant:`), `eos_token` appended
///   only to assistant turns.
const TMPL_DEEPSEEK: &str = concat!(
    "{% if not add_generation_prompt is defined %}",
    "{% set add_generation_prompt = false %}",
    "{% endif %}",
    "{{ bos_token }}",
    "{% for message in messages %}",
    "{% if message['role'] == 'user' %}",
    "{{ 'User: ' + message['content'] + '\\n\\n' }}",
    "{% elif message['role'] == 'assistant' %}",
    "{{ 'Assistant: ' + message['content'] + eos_token }}",
    "{% elif message['role'] == 'system' %}",
    "{{ message['content'] + '\\n\\n' }}",
    "{% endif %}",
    "{% endfor %}",
    "{% if add_generation_prompt %}{{ 'Assistant:' }}{% endif %}"
);

// ── Shared helpers ────────────────────────────────────────────────────────

fn user_msg(content: &str) -> ChatMessage {
    ChatMessage { role: "user".into(), content: content.into() }
}

fn assistant_msg(content: &str) -> ChatMessage {
    ChatMessage { role: "assistant".into(), content: content.into() }
}

fn system_msg(content: &str) -> ChatMessage {
    ChatMessage { role: "system".into(), content: content.into() }
}

fn ctx_with(bos: &str, eos: &str, gen_prompt: bool) -> RenderContext {
    let mut c = RenderContext::new();
    c.set_var("bos_token", bos);
    c.set_var("eos_token", eos);
    c.set_flag("add_generation_prompt", gen_prompt);
    c
}

fn render(tmpl: &str, msgs: &[ChatMessage], ctx: &RenderContext) -> String {
    render_chat_template_with_context(tmpl, msgs, ctx)
}

// ══════════════════════════════════════════════════════════════════════════
// TinyLlama  (arch=llama, eos_token delimiters)
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn tinyllama_single_user_with_gen_prompt() {
    let ctx = ctx_with("<s>", "</s>", true);
    let msgs = [user_msg("Hello there")];
    let out = render(TMPL_TINYLLAMA, &msgs, &ctx);

    assert!(out.contains("<|user|>\nHello there</s>"),
        "expected user turn with eos; got: {out:?}");
    assert!(out.contains("<|assistant|>"),
        "expected generation-prompt marker; got: {out:?}");
    assert!(!out.contains("<|system|>"),
        "no system message should appear; got: {out:?}");
}

#[test]
fn tinyllama_system_then_user_then_assistant_multiturn() {
    let ctx = ctx_with("<s>", "</s>", false);
    let msgs = [
        system_msg("You are a pirate."),
        user_msg("Hello"),
        assistant_msg("Ahoy!"),
        user_msg("What's your name?"),
    ];
    let out = render(TMPL_TINYLLAMA, &msgs, &ctx);

    // Every turn must carry eos_token
    assert!(out.contains("<|system|>\nYou are a pirate.</s>"),
        "system turn; got: {out:?}");
    assert!(out.contains("<|user|>\nHello</s>"),
        "first user turn; got: {out:?}");
    assert!(out.contains("<|assistant|>\nAhoy!</s>"),
        "assistant turn; got: {out:?}");
    assert!(out.contains("<|user|>\nWhat's your name?</s>"),
        "second user turn; got: {out:?}");
    // add_generation_prompt=false → no trailing <|assistant|>
    assert!(!out.ends_with("<|assistant|>"),
        "should NOT end with gen-prompt when flag is false; got: {out:?}");
}

#[test]
fn tinyllama_loop_last_gates_gen_prompt() {
    // loop.last is only true for the final message; the generation-prompt
    // marker must appear exactly once, at the very end.
    let ctx = ctx_with("<s>", "</s>", true);
    let msgs = [user_msg("A"), user_msg("B"), user_msg("C")];
    let out = render(TMPL_TINYLLAMA, &msgs, &ctx);

    let count = out.matches("<|assistant|>").count();
    assert_eq!(count, 1,
        "gen-prompt marker should appear exactly once; got {count} in: {out:?}");
}

// ══════════════════════════════════════════════════════════════════════════
// Gemma 2  (arch=gemma2, bos_token prefix, start/end_of_turn delimiters)
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn gemma2_single_user_with_gen_prompt() {
    let ctx = ctx_with("<bos>", "<eos>", true);
    let msgs = [user_msg("Hello there")];
    let out = render(TMPL_GEMMA2, &msgs, &ctx);

    assert!(out.starts_with("<bos>"),
        "must start with bos_token; got: {out:?}");
    assert!(out.contains("<start_of_turn>user\nHello there<end_of_turn>"),
        "user turn delimiters; got: {out:?}");
    assert!(out.contains("<start_of_turn>model\n"),
        "gen-prompt opens model turn; got: {out:?}");
}

#[test]
fn gemma2_assistant_role_renamed_to_model() {
    // The template renames 'assistant' → 'model' to comply with Gemma naming.
    let ctx = ctx_with("<bos>", "<eos>", false);
    let msgs = [
        user_msg("Hi"),
        assistant_msg("Hello!"),
        user_msg("How are you?"),
    ];
    let out = render(TMPL_GEMMA2, &msgs, &ctx);

    assert!(out.contains("<start_of_turn>model\nHello!<end_of_turn>"),
        "assistant role must be output as 'model'; got: {out:?}");
    assert!(!out.contains("<start_of_turn>assistant"),
        "raw 'assistant' role must never appear; got: {out:?}");
}

#[test]
fn gemma2_system_role_causes_raise_exception_noop() {
    // raise_exception() is a no-op in shimmyjinja (returns empty string).
    // The template should still render the rest of the conversation.
    let ctx = ctx_with("<bos>", "<eos>", true);
    let msgs = [
        system_msg("You are helpful."),
        user_msg("Hi"),
    ];
    let out = render(TMPL_GEMMA2, &msgs, &ctx);
    // Should not panic, and should still have user content
    assert!(out.contains("<start_of_turn>"),
        "render should continue after raise_exception noop; got: {out:?}");
}

#[test]
fn gemma2_alternating_role_check_uses_modulo() {
    // Template checks `(role == 'user') != (loop.index0 % 2 == 0)`.
    // For a properly alternating [user, assistant, user] sequence this is
    // false for every message — no exception triggered.
    let ctx = ctx_with("<bos>", "<eos>", true);
    let msgs = [user_msg("A"), assistant_msg("B"), user_msg("C")];
    let out = render(TMPL_GEMMA2, &msgs, &ctx);
    assert!(out.contains("<start_of_turn>user\nA<end_of_turn>"),
        "first user turn; got: {out:?}");
    assert!(out.contains("<start_of_turn>model\nB<end_of_turn>"),
        "assistant (→model) turn; got: {out:?}");
    assert!(out.contains("<start_of_turn>user\nC<end_of_turn>"),
        "second user turn; got: {out:?}");
}

// ══════════════════════════════════════════════════════════════════════════
// Phi-3.5-mini  (arch=phi3, system-role support, <|end|> delimiter)
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn phi35_user_with_gen_prompt() {
    let ctx = ctx_with("<s>", "<|endoftext|>", true);
    let msgs = [user_msg("Hello there")];
    let out = render(TMPL_PHI35, &msgs, &ctx);

    assert!(out.contains("<|user|>\nHello there<|end|>"),
        "user turn with <|end|>; got: {out:?}");
    assert!(out.contains("<|assistant|>\n"),
        "gen-prompt appends assistant marker; got: {out:?}");
}

#[test]
fn phi35_system_message_rendered_when_content_nonempty() {
    let ctx = ctx_with("<s>", "<|endoftext|>", true);
    let msgs = [system_msg("You are helpful."), user_msg("Hi")];
    let out = render(TMPL_PHI35, &msgs, &ctx);

    assert!(out.contains("<|system|>\nYou are helpful.<|end|>"),
        "system turn; got: {out:?}");
    assert!(out.contains("<|user|>\nHi<|end|>"),
        "user turn after system; got: {out:?}");
}

#[test]
fn phi35_no_gen_prompt_emits_eos_token() {
    // When add_generation_prompt=false, the else branch emits eos_token.
    let ctx = ctx_with("<s>", "<|endoftext|>", false);
    let msgs = [user_msg("Hello")];
    let out = render(TMPL_PHI35, &msgs, &ctx);

    assert!(out.ends_with("<|endoftext|>"),
        "should end with eos_token when add_generation_prompt=false; got: {out:?}");
}

// ══════════════════════════════════════════════════════════════════════════
// Phi-3-mini-4k  (arch=phi3, bos_token prefix, user→assistant in one shot)
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn phi3mini_single_user() {
    let ctx = ctx_with("<s>", "<|endoftext|>", false);
    let msgs = [user_msg("Hello there")];
    let out = render(TMPL_PHI3MINI, &msgs, &ctx);

    assert!(out.starts_with("<s>"),
        "must start with bos_token; got: {out:?}");
    assert!(out.contains("<|user|>\nHello there<|end|>\n<|assistant|>"),
        "user turn directly followed by assistant header; got: {out:?}");
}

#[test]
fn phi3mini_multiturn_assistant_turn_has_end_delimiter() {
    let ctx = ctx_with("<s>", "<|endoftext|>", false);
    let msgs = [user_msg("Q1"), assistant_msg("A1"), user_msg("Q2")];
    let out = render(TMPL_PHI3MINI, &msgs, &ctx);

    assert!(out.starts_with("<s>"),
        "bos_token; got: {out:?}");
    assert!(out.contains("A1<|end|>"),
        "assistant turn closes with <|end|>; got: {out:?}");
}

// ══════════════════════════════════════════════════════════════════════════
// Qwen2 / Qwen2.5  (arch=qwen2, ChatML, implicit system injection)
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn qwen2_user_only_injects_default_system() {
    // Template checks `loop.first and messages[0]['role'] != 'system'` to
    // decide whether to prepend the default system message.
    let ctx = ctx_with("<|endoftext|>", "<|im_end|>", true);
    let msgs = [user_msg("Hello there")];
    let out = render(TMPL_QWEN2, &msgs, &ctx);

    assert!(out.contains("You are a helpful assistant."),
        "default system message must be injected; got: {out:?}");
    assert!(out.contains("<|im_start|>user\nHello there<|im_end|>"),
        "user turn in ChatML format; got: {out:?}");
    assert!(out.ends_with("<|im_start|>assistant\n"),
        "gen-prompt opens assistant turn; got: {out:?}");
}

#[test]
fn qwen2_explicit_system_suppresses_default_injection() {
    let ctx = ctx_with("<|endoftext|>", "<|im_end|>", true);
    let msgs = [system_msg("Custom system."), user_msg("Hello")];
    let out = render(TMPL_QWEN2, &msgs, &ctx);

    // Default system must NOT appear when an explicit system message is given
    assert!(!out.contains("You are a helpful assistant."),
        "default system must not appear when explicit system provided; got: {out:?}");
    assert!(out.contains("<|im_start|>system\nCustom system.<|im_end|>"),
        "explicit system turn; got: {out:?}");
    assert!(out.contains("<|im_start|>user\nHello<|im_end|>"),
        "user turn; got: {out:?}");
}

#[test]
fn qwen2_multiturn_all_roles_present() {
    let ctx = ctx_with("<|endoftext|>", "<|im_end|>", false);
    let msgs = [
        system_msg("Be concise."),
        user_msg("What is 2+2?"),
        assistant_msg("4"),
        user_msg("And 3+3?"),
    ];
    let out = render(TMPL_QWEN2, &msgs, &ctx);

    assert!(out.contains("<|im_start|>system\nBe concise.<|im_end|>"),  "system");
    assert!(out.contains("<|im_start|>user\nWhat is 2+2?<|im_end|>"),   "user 1");
    assert!(out.contains("<|im_start|>assistant\n4<|im_end|>"),          "assistant");
    assert!(out.contains("<|im_start|>user\nAnd 3+3?<|im_end|>"),       "user 2");
}

// ══════════════════════════════════════════════════════════════════════════
// DeepSeek-LLM-7B  (arch=llama, plain-text User:/Assistant: style)
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn deepseek_user_with_gen_prompt() {
    // Real bos/eos tokens — the distinctive DeepSeek Unicode delimiters.
    // U+FF5C = ｜ (FULLWIDTH VERTICAL LINE), U+2581 = ▁ (LOWER ONE EIGHTH BLOCK)
    let bos = "\u{FF5C}begin\u{2581}of\u{2581}sentence\u{FF5C}";
    let eos = "\u{FF5C}end\u{2581}of\u{2581}sentence\u{FF5C}";
    let ctx = ctx_with(bos, eos, true);
    let msgs = [user_msg("Hello there")];
    let out = render(TMPL_DEEPSEEK, &msgs, &ctx);

    assert!(out.contains(bos),
        "must start with bos_token; got: {out:?}");
    assert!(out.contains("User: Hello there\n\n"),
        "user turn in plain-text style; got: {out:?}");
    assert!(out.ends_with("Assistant:"),
        "gen-prompt appends bare 'Assistant:'; got: {out:?}");
}

#[test]
fn deepseek_assistant_turn_has_eos() {
    let bos = "\u{FF5C}begin\u{25B1}of\u{25B1}sentence\u{FF5C}";
    let eos = "\u{FF5C}end\u{25B1}of\u{25B1}sentence\u{FF5C}";
    let ctx = ctx_with(bos, eos, false);
    let msgs = [
        user_msg("Hello"),
        assistant_msg("Hi there"),
        user_msg("What's up?"),
    ];
    let out = render(TMPL_DEEPSEEK, &msgs, &ctx);

    // Assistant turns carry eos_token immediately after content
    let expected_assistant = format!("Assistant: Hi there{eos}");
    assert!(out.contains(&expected_assistant),
        "assistant turn must end with eos; got: {out:?}");
    // User turns must NOT carry eos
    assert!(out.contains("User: What's up?\n\n"),
        "user turns use plain double-newline; got: {out:?}");
}

#[test]
fn deepseek_define_guard_does_not_overwrite_caller_value() {
    // `{% if not add_generation_prompt is defined %}` — the guard must only
    // set the default when the flag is *absent* from the context.
    // Here we pass add_generation_prompt=false; the guard body must NOT run.
    let bos = "\u{FF5C}begin\u{25B1}of\u{25B1}sentence\u{FF5C}";
    let eos = "\u{FF5C}end\u{25B1}of\u{25B1}sentence\u{FF5C}";
    let ctx = ctx_with(bos, eos, false);
    let msgs = [user_msg("Ping")];
    let out = render(TMPL_DEEPSEEK, &msgs, &ctx);

    assert!(!out.ends_with("Assistant:"),
        "add_generation_prompt=false should suppress the suffix; got: {out:?}");
}

// ══════════════════════════════════════════════════════════════════════════
// Cross-model regression: features shared by multiple templates
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn trim_filter_strips_leading_trailing_whitespace() {
    // Gemma2 uses `message['content'] | trim` — verify trim works in concat.
    let ctx = ctx_with("<bos>", "<eos>", false);
    let msgs = [user_msg("  hello  ")];
    let out = render(TMPL_GEMMA2, &msgs, &ctx);
    assert!(out.contains("<start_of_turn>user\nhello<end_of_turn>"),
        "|trim must remove surrounding whitespace; got: {out:?}");
}

#[test]
fn bos_token_prefix_appears_at_start() {
    // Both Phi3-mini and Gemma2 prepend bos_token via `{{ bos_token }}`.
    for (tmpl, bos) in [
        (TMPL_PHI3MINI, "<s>"),
        (TMPL_GEMMA2,   "<bos>"),
    ] {
        let ctx = ctx_with(bos, "</s>", false);
        let msgs = [user_msg("Test")];
        let out = render(tmpl, &msgs, &ctx);
        assert!(out.starts_with(bos),
            "bos_token must be the very first character; got: {out:?}");
    }
}

#[test]
fn eos_token_is_interpolated_not_literal() {
    // TinyLlama uses `+ eos_token` in the expression — verify it receives the
    // actual runtime value, not the string "eos_token".
    let ctx = ctx_with("<s>", "<<EOS_SENTINEL>>", true);
    let msgs = [user_msg("Test")];
    let out = render(TMPL_TINYLLAMA, &msgs, &ctx);
    assert!(out.contains("<<EOS_SENTINEL>>"),
        "eos_token variable must be substituted with the runtime value; got: {out:?}");
    assert!(!out.contains("eos_token\""),
        "literal string 'eos_token' must never appear unresolved; got: {out:?}");
}
