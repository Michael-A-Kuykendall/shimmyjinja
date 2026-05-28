//! Integration tests using real Hugging Face chat_template strings.
//! No model files are required — these tests run on raw Jinja strings only.

use shimmyjinja::{render_chat_template_with_context, ChatMessage, RenderContext};

// ── helpers ────────────────────────────────────────────────────────────────

fn user(content: &str) -> ChatMessage {
    ChatMessage { role: "user".into(), content: content.into() }
}
fn assistant(content: &str) -> ChatMessage {
    ChatMessage { role: "assistant".into(), content: content.into() }
}
fn system(content: &str) -> ChatMessage {
    ChatMessage { role: "system".into(), content: content.into() }
}

fn ctx(bos: &str, eos: &str, add_gen: bool) -> RenderContext {
    let mut c = RenderContext::new();
    c.set_var("bos_token", bos);
    c.set_var("eos_token", eos);
    c.set_flag("add_generation_prompt", add_gen);
    c
}

// ── ChatML / Qwen ──────────────────────────────────────────────────────────

/// The canonical ChatML template used by ChatML-based models (Qwen, etc.)
#[test]
fn chatml_basic() {
    let template = concat!(
        "{% for message in messages %}",
        "{{'<|im_start|>' + message['role'] + '\\n' + message['content'] + '<|im_end|>' + '\\n'}}",
        "{% endfor %}",
        "{% if add_generation_prompt %}",
        "{{'<|im_start|>assistant\\n'}}",
        "{% endif %}"
    );

    let messages = vec![system("You are a helpful assistant."), user("Hello!")];
    let rendered = render_chat_template_with_context(template, &messages, &ctx("", "", true));

    assert!(rendered.contains("<|im_start|>system\nYou are a helpful assistant.<|im_end|>"));
    assert!(rendered.contains("<|im_start|>user\nHello!<|im_end|>"));
    assert!(rendered.trim_end().ends_with("<|im_start|>assistant"));
}

#[test]
fn chatml_no_generation_prompt() {
    let template = concat!(
        "{% for message in messages %}",
        "{{'<|im_start|>' + message['role'] + '\\n' + message['content'] + '<|im_end|>' + '\\n'}}",
        "{% endfor %}",
        "{% if add_generation_prompt %}",
        "{{'<|im_start|>assistant\\n'}}",
        "{% endif %}"
    );

    let messages = vec![user("Hi")];
    let rendered = render_chat_template_with_context(template, &messages, &ctx("", "", false));

    assert!(rendered.contains("<|im_start|>user"));
    assert!(!rendered.contains("assistant"), "Should not have assistant prompt");
}

#[test]
fn chatml_trim_filter_on_content() {
    let template = concat!(
        "{% for message in messages %}",
        "{{'<|im_start|>' + message['role'] + '\\n' + message['content'] | trim + '<|im_end|>\\n'}}",
        "{% endfor %}"
    );

    // Content has leading/trailing whitespace — | trim should strip it
    let messages = vec![ChatMessage {
        role: "user".into(),
        content: "  hello world  ".into(),
    }];
    let rendered = render_chat_template_with_context(template, &messages, &ctx("", "", false));
    assert!(rendered.contains("hello world<|im_end|>"), "trim should strip whitespace: {}", rendered);
    assert!(!rendered.contains("  hello"), "leading spaces should be gone");
}

// ── Llama 3 ────────────────────────────────────────────────────────────────

/// Llama 3 Instruct template — uses {% set %}, loop.first, | trim, !=
#[test]
fn llama3_with_system() {
    let template = concat!(
        "{% set loop_messages = messages %}",
        "{% for message in loop_messages %}",
            "{% set content = '<|start_header_id|>' + message['role'] + '<|end_header_id|>\\n\\n'",
                            "+ message['content'] | trim + '<|eot_id|>' %}",
            "{% if loop.first and messages[0]['role'] != 'system' %}",
                "{% set content = bos_token + content %}",
            "{% endif %}",
            "{{ content }}",
        "{% endfor %}",
        "{% if add_generation_prompt %}",
            "{{ '<|start_header_id|>assistant<|end_header_id|>\\n\\n' }}",
        "{% endif %}"
    );

    let messages = vec![
        system("You are a helpful AI."),
        user("What is 2+2?"),
    ];
    let rendered = render_chat_template_with_context(
        template, &messages, &ctx("<|begin_of_text|>", "<|end_of_text|>", true),
    );

    // System message appears first — bos_token injection is skipped because
    // messages[0]['role'] IS 'system'
    assert!(rendered.contains("<|start_header_id|>system<|end_header_id|>"),
        "system header: {}", rendered);
    assert!(rendered.contains("You are a helpful AI."), "system content: {}", rendered);
    assert!(rendered.contains("<|start_header_id|>user<|end_header_id|>"),
        "user header: {}", rendered);
    assert!(rendered.contains("What is 2+2?"), "user content: {}", rendered);
    assert!(rendered.contains("<|start_header_id|>assistant<|end_header_id|>"),
        "generation prompt: {}", rendered);
}

#[test]
fn llama3_no_system_bos_injected() {
    let template = concat!(
        "{% set loop_messages = messages %}",
        "{% for message in loop_messages %}",
            "{% set content = '<|start_header_id|>' + message['role'] + '<|end_header_id|>\\n\\n'",
                            "+ message['content'] | trim + '<|eot_id|>' %}",
            "{% if loop.first and messages[0]['role'] != 'system' %}",
                "{% set content = bos_token + content %}",
            "{% endif %}",
            "{{ content }}",
        "{% endfor %}",
        "{% if add_generation_prompt %}",
            "{{ '<|start_header_id|>assistant<|end_header_id|>\\n\\n' }}",
        "{% endif %}"
    );

    // First message is user, not system → bos_token should be prepended
    let messages = vec![user("Hello!")];
    let rendered = render_chat_template_with_context(
        template, &messages, &ctx("<|begin_of_text|>", "<|end_of_text|>", true),
    );

    assert!(rendered.starts_with("<|begin_of_text|>"),
        "bos_token must be first: {:?}", rendered);
    assert!(rendered.contains("Hello!"), "content present: {}", rendered);
}

#[test]
fn set_statement_basic() {
    let template = concat!(
        "{% set greeting = 'Hello' %}",
        "{{ greeting }}, world!"
    );
    let rendered = render_chat_template_with_context(template, &[], &RenderContext::new());
    assert_eq!(rendered.trim(), "Hello, world!");
}

#[test]
fn set_statement_reassign_inside_loop() {
    let template = concat!(
        "{% for message in messages %}",
            "{% set text = message['role'] + ': ' + message['content'] %}",
            "{{ text }}\\n",
        "{% endfor %}"
    );
    let messages = vec![user("hi"), assistant("hello")];
    let rendered = render_chat_template_with_context(template, &messages, &RenderContext::new());
    assert!(rendered.contains("user: hi"), "user line: {}", rendered);
    assert!(rendered.contains("assistant: hello"), "assistant line: {}", rendered);
}

// ── Mistral ────────────────────────────────────────────────────────────────

/// Simplified Mistral template — uses bos_token, eos_token, != comparison,
/// raise_exception (no-op), elif
#[test]
fn mistral_basic() {
    let template = concat!(
        "{{ bos_token }}",
        "{% for message in messages %}",
            "{% if message['role'] == 'user' %}",
                "{{ '[INST] ' + message['content'] + ' [/INST]' }}",
            "{% elif message['role'] == 'assistant' %}",
                "{{ message['content'] + eos_token }}",
            "{% else %}",
                "{{ raise_exception('Only user and assistant roles are supported!') }}",
            "{% endif %}",
        "{% endfor %}"
    );

    let messages = vec![user("What is Rust?"), assistant("A systems language.")];
    let rendered = render_chat_template_with_context(
        template, &messages, &ctx("<s>", "</s>", false),
    );

    assert!(rendered.starts_with("<s>"), "bos_token: {}", rendered);
    assert!(rendered.contains("[INST] What is Rust? [/INST]"), "user formatted: {}", rendered);
    assert!(rendered.contains("A systems language.</s>"), "assistant formatted: {}", rendered);
}

#[test]
fn raise_exception_is_noop() {
    // Templates sometimes call raise_exception in an else branch.
    // It should produce no output, not crash.
    let template = concat!(
        "{% for message in messages %}",
            "{% if message['role'] == 'user' %}",
                "{{ message['content'] }}",
            "{% else %}",
                "{{ raise_exception('Unexpected role') }}",
            "{% endif %}",
        "{% endfor %}"
    );
    let messages = vec![user("hello"), system("system prompt")];
    let rendered = render_chat_template_with_context(template, &messages, &RenderContext::new());
    assert_eq!(rendered, "hello");
}

// ── Gemma ──────────────────────────────────────────────────────────────────

/// Gemma 2 template — uses bos_token, | trim, elif for model role
#[test]
fn gemma2_basic() {
    let template = concat!(
        "{{ bos_token }}",
        "{% for message in messages %}",
            "{% if message['role'] == 'user' %}",
                "{{'<start_of_turn>user\\n' + message['content'] | trim + '<end_of_turn>\\n'}}",
            "{% elif message['role'] == 'assistant' %}",
                "{{'<start_of_turn>model\\n' + message['content'] | trim + '<end_of_turn>\\n'}}",
            "{% endif %}",
        "{% endfor %}",
        "{% if add_generation_prompt %}",
            "{{'<start_of_turn>model\\n'}}",
        "{% endif %}"
    );

    let messages = vec![user("  Hello Gemma!  "), assistant("  Hi there!  ")];
    let rendered = render_chat_template_with_context(
        template, &messages, &ctx("<bos>", "<eos>", true),
    );

    assert!(rendered.starts_with("<bos>"), "bos_token: {}", rendered);
    assert!(rendered.contains("<start_of_turn>user\nHello Gemma!<end_of_turn>"),
        "user trimmed: {}", rendered);
    assert!(rendered.contains("<start_of_turn>model\nHi there!<end_of_turn>"),
        "assistant trimmed: {}", rendered);
    assert!(rendered.trim_end().ends_with("<start_of_turn>model"),
        "generation prompt: {}", rendered);
}

// ── Operator tests ─────────────────────────────────────────────────────────

#[test]
fn ne_operator_string() {
    let template = concat!(
        "{% if messages[0]['role'] != 'system' %}",
            "no system",
        "{% else %}",
            "has system",
        "{% endif %}"
    );
    let messages = vec![user("hi")];
    let rendered = render_chat_template_with_context(template, &messages, &RenderContext::new());
    assert_eq!(rendered.trim(), "no system");
}

#[test]
fn ne_operator_bool() {
    // (a == b) != (c == d) — Mistral-style guard
    let template = concat!(
        "{% if (messages[0]['role'] == 'user') != (messages[1]['role'] == 'user') %}",
            "mismatch",
        "{% else %}",
            "match",
        "{% endif %}"
    );
    // Both are 'user' — (true) != (true) → false → "match"
    let messages = vec![user("a"), user("b")];
    let rendered = render_chat_template_with_context(template, &messages, &RenderContext::new());
    assert_eq!(rendered.trim(), "match");
}

#[test]
fn not_operator() {
    let template = "{% if not add_generation_prompt %}skip{% else %}go{% endif %}";
    let mut c = RenderContext::new();
    c.set_flag("add_generation_prompt", false);
    let rendered = render_chat_template_with_context(template, &[], &c);
    assert_eq!(rendered, "skip");
}

// ── Filter tests ───────────────────────────────────────────────────────────

#[test]
fn trim_filter_strips_whitespace() {
    let template = "{{ value | trim }}";
    let mut c = RenderContext::new();
    c.set_var("value", "  hello  ");
    let rendered = render_chat_template_with_context(template, &[], &c);
    assert_eq!(rendered, "hello");
}

#[test]
fn default_filter_on_null() {
    // 'missing' is not in context so it evaluates to Null → default kicks in
    let template = "{{ missing | default('fallback') }}";
    let rendered = render_chat_template_with_context(template, &[], &RenderContext::new());
    assert_eq!(rendered, "fallback");
}

#[test]
fn default_filter_on_present_value() {
    let template = "{{ eos_token | default('</s>') }}";
    let mut c = RenderContext::new();
    c.set_var("eos_token", "<|endoftext|>");
    let rendered = render_chat_template_with_context(template, &[], &c);
    assert_eq!(rendered, "<|endoftext|>");
}

// ── Negative indexing ──────────────────────────────────────────────────────

#[test]
fn negative_array_index() {
    // messages[-1] should get the last message
    let template = "{{ messages[-1]['content'] }}";
    let messages = vec![user("first"), user("last message")];
    let rendered = render_chat_template_with_context(template, &messages, &RenderContext::new());
    assert_eq!(rendered, "last message");
}

#[test]
fn zero_index_access() {
    let template = "{{ messages[0]['role'] }}";
    let messages = vec![system("sys"), user("usr")];
    let rendered = render_chat_template_with_context(template, &messages, &RenderContext::new());
    assert_eq!(rendered, "system");
}

// ── Whitespace control (`{%-` / `-%}`) ────────────────────────────────────

#[test]
fn trim_block_start_strips_preceding_whitespace() {
    // {%- strips trailing whitespace/newlines from preceding text
    let template = "before   {%- if true %}inside{% endif %}";
    let rendered = render_chat_template_with_context(template, &[], &RenderContext::new());
    assert_eq!(rendered, "beforeinside");
}

#[test]
fn trim_block_end_strips_following_whitespace() {
    // -%} strips leading whitespace/newlines from following text
    let template = "{% if true -%}   after{% endif %}";
    let rendered = render_chat_template_with_context(template, &[], &RenderContext::new());
    assert_eq!(rendered, "after");
}

// ── loop.index / loop.first / loop.last ───────────────────────────────────

#[test]
fn loop_index0_is_integer() {
    // loop.index0 == 0 should be truthy for the first iteration
    let template = concat!(
        "{% for message in messages %}",
            "{% if loop.index0 == 0 %}FIRST{% endif %}",
            "{{ message['content'] }}",
        "{% endfor %}"
    );
    let messages = vec![user("a"), user("b")];
    let rendered = render_chat_template_with_context(template, &messages, &RenderContext::new());
    assert!(rendered.contains("FIRSTa"), "first iter: {}", rendered);
    assert!(!rendered.contains("FIRSTb"), "only first: {}", rendered);
}
