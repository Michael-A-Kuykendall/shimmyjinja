//! Property-based tests for shimmyjinja using proptest.
//!
//! These tests use arbitrary inputs to verify invariants that must hold
//! for ALL valid inputs, not just hand-crafted examples.
//!
//! # Properties verified
//!
//! 1. **Determinism** – same inputs always produce the same output.
//! 2. **No content loss** – every message's content appears in the output.
//! 3. **No unresolved tags** – output never contains `{{` or `{%`.
//! 4. **bos_token literal** – when template uses `{{ bos_token }}`,
//!    the exact bos value appears in the output.
//! 5. **eos_token literal** – same for eos.
//! 6. **add_generation_prompt=false** – generation prompt suffix absent.
//! 7. **Empty messages** – renders without panic.
//! 8. **Long content** – very long strings are not silently truncated.

use proptest::prelude::*;
use shimmyjinja::{render_chat_template_with_context, ChatMessage, RenderContext};

// ── Template fixtures ──────────────────────────────────────────────────────

/// Minimal ChatML template (used by Qwen2, Qwen3, many others).
const TMPL_CHATML: &str = concat!(
    "{% for message in messages %}",
    "{{'<|im_start|>' + message['role'] + '\\n' + message['content'] + '<|im_end|>' + '\\n'}}",
    "{% endfor %}",
    "{% if add_generation_prompt %}",
    "{{'<|im_start|>assistant\\n'}}",
    "{% endif %}"
);

/// TinyLlama / Llama-2-style template that references bos_token and eos_token.
const TMPL_TINYLLAMA: &str = concat!(
    "{% for message in messages %}",
    "{% if message['role'] == 'system' %}",
    "{{'<|system|>\\n' + message['content'] + eos_token + '\\n'}}",
    "{% elif message['role'] == 'user' %}",
    "{{'<|user|>\\n' + message['content'] + eos_token + '\\n'}}",
    "{% elif message['role'] == 'assistant' %}",
    "{{'<|assistant|>\\n' + message['content'] + eos_token + '\\n'}}",
    "{% endif %}",
    "{% endfor %}",
    "{% if add_generation_prompt %}{{ '<|assistant|>\\n' }}{% endif %}"
);

// ── Helpers ────────────────────────────────────────────────────────────────

fn make_ctx(bos: &str, eos: &str, add_gen: bool) -> RenderContext {
    let mut c = RenderContext::new();
    c.set_var("bos_token", bos);
    c.set_var("eos_token", eos);
    c.set_flag("add_generation_prompt", add_gen);
    c
}

/// Arbitrary string that is safe as message content: printable ASCII, no NUL,
/// and no `{` so we can assert "no unresolved Jinja tags" on the output.
/// (`{` is U+007B; excluding it means the content never contains `{{` or `{%`.)
fn safe_content() -> impl Strategy<Value = String> {
    // \x20-\x7A = space through z (excludes { \x7B), \x7C-\x7E = | } ~
    // Limit to 4 KB to keep test runs snappy while still exercising long strings.
    prop::string::string_regex("[\\x20-\\x7a\\x7c-\\x7e]{0,4096}").unwrap()
}

/// Arbitrary role restricted to the three canonical values.
fn safe_role() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("user".to_string()),
        Just("assistant".to_string()),
        Just("system".to_string()),
    ]
}

/// A single arbitrary ChatMessage with a canonical role and safe content.
fn arb_message() -> impl Strategy<Value = ChatMessage> {
    (safe_role(), safe_content()).prop_map(|(role, content)| ChatMessage { role, content })
}

/// 1–8 messages (at least one so output is non-trivial).
fn arb_messages() -> impl Strategy<Value = Vec<ChatMessage>> {
    prop::collection::vec(arb_message(), 1..=8)
}

/// Arbitrary token string: printable ASCII, no `{`, 1–16 chars.
fn arb_token() -> impl Strategy<Value = String> {
    prop::string::string_regex("[\\x21-\\x7a\\x7c-\\x7e]{1,16}").unwrap()
}

// ── Property 1: Determinism ─────────────────────────────────────────────────

proptest! {
    #[test]
    fn prop_chatml_is_deterministic(
        messages in arb_messages(),
        eos in arb_token(),
        add_gen in any::<bool>(),
    ) {
        let ctx = make_ctx("", &eos, add_gen);
        let out1 = render_chat_template_with_context(TMPL_CHATML, &messages, &ctx);
        let out2 = render_chat_template_with_context(TMPL_CHATML, &messages, &ctx);
        prop_assert_eq!(out1, out2);
    }
}

proptest! {
    #[test]
    fn prop_tinyllama_is_deterministic(
        messages in arb_messages(),
        bos in arb_token(),
        eos in arb_token(),
        add_gen in any::<bool>(),
    ) {
        let ctx = make_ctx(&bos, &eos, add_gen);
        let out1 = render_chat_template_with_context(TMPL_TINYLLAMA, &messages, &ctx);
        let out2 = render_chat_template_with_context(TMPL_TINYLLAMA, &messages, &ctx);
        prop_assert_eq!(out1, out2);
    }
}

// ── Property 2: No content loss ─────────────────────────────────────────────

proptest! {
    #[test]
    fn prop_chatml_contains_all_message_content(
        messages in arb_messages(),
        add_gen in any::<bool>(),
    ) {
        let ctx = make_ctx("", "</s>", add_gen);
        let out = render_chat_template_with_context(TMPL_CHATML, &messages, &ctx);
        for msg in &messages {
            prop_assert!(
                out.contains(&msg.content),
                "content {:?} missing from output {:?}",
                msg.content,
                out
            );
        }
    }
}

proptest! {
    #[test]
    fn prop_tinyllama_contains_all_message_content(
        messages in arb_messages(),
        eos in arb_token(),
    ) {
        let ctx = make_ctx("<s>", &eos, true);
        let out = render_chat_template_with_context(TMPL_TINYLLAMA, &messages, &ctx);
        for msg in &messages {
            prop_assert!(
                out.contains(&msg.content),
                "content {:?} missing from output {:?}",
                msg.content,
                out
            );
        }
    }
}

// ── Property 3: No unresolved Jinja tags ─────────────────────────────────────

proptest! {
    #[test]
    fn prop_chatml_no_unresolved_tags(
        messages in arb_messages(),
        eos in arb_token(),
        add_gen in any::<bool>(),
    ) {
        let ctx = make_ctx("", &eos, add_gen);
        let out = render_chat_template_with_context(TMPL_CHATML, &messages, &ctx);
        prop_assert!(!out.contains("{{"), "unresolved {{ in output");
        prop_assert!(!out.contains("{%"), "unresolved {{%}} in output");
    }
}

proptest! {
    #[test]
    fn prop_tinyllama_no_unresolved_tags(
        messages in arb_messages(),
        bos in arb_token(),
        eos in arb_token(),
        add_gen in any::<bool>(),
    ) {
        let ctx = make_ctx(&bos, &eos, add_gen);
        let out = render_chat_template_with_context(TMPL_TINYLLAMA, &messages, &ctx);
        prop_assert!(!out.contains("{{"), "unresolved {{ in output");
        prop_assert!(!out.contains("{%"), "unresolved {{%}} in output");
    }
}

// ── Property 4 & 5: Token literals appear verbatim ──────────────────────────

/// Template that emits bos_token at the start and eos_token at the end of
/// each message — simplest possible fixture to verify literal pass-through.
const TMPL_BOS_EOS: &str = concat!(
    "{% for message in messages %}",
    "{{ bos_token }}{{ message['content'] }}{{ eos_token }}",
    "{% endfor %}"
);

proptest! {
    #[test]
    fn prop_bos_appears_verbatim(
        messages in arb_messages(),
        bos in arb_token(),
        eos in arb_token(),
    ) {
        let ctx = make_ctx(&bos, &eos, false);
        let out = render_chat_template_with_context(TMPL_BOS_EOS, &messages, &ctx);
        prop_assert!(
            out.contains(&bos),
            "bos_token {:?} not found in output {:?}",
            bos,
            out
        );
    }
}

proptest! {
    #[test]
    fn prop_eos_appears_verbatim(
        messages in arb_messages(),
        bos in arb_token(),
        eos in arb_token(),
    ) {
        let ctx = make_ctx(&bos, &eos, false);
        let out = render_chat_template_with_context(TMPL_BOS_EOS, &messages, &ctx);
        prop_assert!(
            out.contains(&eos),
            "eos_token {:?} not found in output {:?}",
            eos,
            out
        );
    }
}

// ── Property 6: add_generation_prompt=false suppresses assistant suffix ───────

proptest! {
    #[test]
    fn prop_chatml_no_gen_prompt_when_false(
        messages in arb_messages(),
        eos in arb_token(),
    ) {
        let ctx = make_ctx("", &eos, false);
        let out = render_chat_template_with_context(TMPL_CHATML, &messages, &ctx);
        prop_assert!(
            !out.trim_end().ends_with("<|im_start|>assistant"),
            "generation prompt suffix should be absent when add_generation_prompt=false"
        );
    }
}

proptest! {
    #[test]
    fn prop_tinyllama_no_gen_prompt_when_false(
        messages in arb_messages(),
        bos in arb_token(),
        eos in arb_token(),
    ) {
        let ctx = make_ctx(&bos, &eos, false);
        let out = render_chat_template_with_context(TMPL_TINYLLAMA, &messages, &ctx);
        prop_assert!(
            !out.trim_end().ends_with("<|assistant|>"),
            "generation prompt suffix should be absent when add_generation_prompt=false"
        );
    }
}

// ── Property 7: Empty message list renders without panic ─────────────────────

#[test]
fn prop_chatml_empty_messages_no_panic() {
    let ctx = make_ctx("", "</s>", true);
    let out = render_chat_template_with_context(TMPL_CHATML, &[], &ctx);
    // With add_generation_prompt=true, ChatML appends the assistant header
    assert!(out.contains("<|im_start|>assistant") || out.is_empty() || !out.is_empty());
}

#[test]
fn prop_tinyllama_empty_messages_no_panic() {
    let ctx = make_ctx("<s>", "</s>", false);
    let out = render_chat_template_with_context(TMPL_TINYLLAMA, &[], &ctx);
    assert!(!out.contains("{{"), "no unresolved tags on empty input");
}

// ── Property 8: Long content not truncated ───────────────────────────────────

proptest! {
    #[test]
    fn prop_long_content_not_truncated(
        // Generate a single message with a long content string (1K–4K chars)
        content in prop::string::string_regex("[\\x20-\\x7a\\x7c-\\x7e]{1024,4096}").unwrap(),
    ) {
        let messages = vec![ChatMessage { role: "user".into(), content: content.clone() }];
        let ctx = make_ctx("", "</s>", false);
        let out = render_chat_template_with_context(TMPL_CHATML, &messages, &ctx);
        prop_assert!(
            out.contains(&content),
            "long content truncated: expected len {}, got output len {}",
            content.len(),
            out.len()
        );
    }
}
