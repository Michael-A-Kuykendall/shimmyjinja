use shimmyjinja::{render_chat_template_with_context, ChatMessage, RenderContext};

// ── Edge cases for crates.io publishing confidence ──

#[test]
fn empty_messages_produces_empty_output() {
    let template = "{% for message in messages %}{{ message.content }}{% endfor %}";
    let messages: Vec<ChatMessage> = vec![];
    let ctx = RenderContext::new();
    let rendered = render_chat_template_with_context(template, &messages, &ctx);
    assert_eq!(rendered, "");
}

#[test]
fn plain_text_template_no_tags() {
    let template = "Hello, world!";
    let messages: Vec<ChatMessage> = vec![];
    let ctx = RenderContext::new();
    let rendered = render_chat_template_with_context(template, &messages, &ctx);
    assert_eq!(rendered, "Hello, world!");
}

#[test]
fn context_var_outside_loop() {
    let template = "{{ bos_token }}PROMPT{{ eos_token }}";
    let messages: Vec<ChatMessage> = vec![];
    let mut ctx = RenderContext::new();
    ctx.set_var("bos_token", "<s>");
    ctx.set_var("eos_token", "</s>");
    let rendered = render_chat_template_with_context(template, &messages, &ctx);
    assert_eq!(rendered, "<s>PROMPT</s>");
}

#[test]
fn dot_access_and_bracket_access_equivalent() {
    let template_dot = "{% for message in messages %}{{ message.role }}{% endfor %}";
    let template_bracket = "{% for message in messages %}{{ message['role'] }}{% endfor %}";
    let messages = vec![
        ChatMessage { role: "user".to_string(), content: "hi".to_string() },
    ];
    let ctx = RenderContext::new();
    let a = render_chat_template_with_context(template_dot, &messages, &ctx);
    let b = render_chat_template_with_context(template_bracket, &messages, &ctx);
    assert_eq!(a, b);
    assert_eq!(a, "user");
}

#[test]
fn loop_first_and_last_single_message() {
    // With only one message, loop.first AND loop.last should both be true
    let template = "{% for message in messages %}{% if loop.first %}F{% endif %}{% if loop.last %}L{% endif %}{% endfor %}";
    let messages = vec![
        ChatMessage { role: "user".to_string(), content: "x".to_string() },
    ];
    let ctx = RenderContext::new();
    let rendered = render_chat_template_with_context(template, &messages, &ctx);
    assert_eq!(rendered, "FL");
}

#[test]
fn loop_first_and_last_multiple_messages() {
    let template = "{% for message in messages %}{% if loop.first %}[{% endif %}{{ message.role }}{% if loop.last %}]{% endif %}{% endfor %}";
    let messages = vec![
        ChatMessage { role: "a".to_string(), content: "".to_string() },
        ChatMessage { role: "b".to_string(), content: "".to_string() },
        ChatMessage { role: "c".to_string(), content: "".to_string() },
    ];
    let ctx = RenderContext::new();
    let rendered = render_chat_template_with_context(template, &messages, &ctx);
    assert_eq!(rendered, "[abc]");
}

#[test]
fn or_operator_in_condition() {
    let template = "{% for message in messages %}{% if message.role == 'user' or message.role == 'assistant' %}Y{% else %}N{% endif %}{% endfor %}";
    let messages = vec![
        ChatMessage { role: "system".to_string(), content: "".to_string() },
        ChatMessage { role: "user".to_string(), content: "".to_string() },
        ChatMessage { role: "assistant".to_string(), content: "".to_string() },
    ];
    let ctx = RenderContext::new();
    let rendered = render_chat_template_with_context(template, &messages, &ctx);
    assert_eq!(rendered, "NYY");
}

#[test]
fn string_concat_multiple_parts() {
    let template = "{% for message in messages %}{{ 'A' + 'B' + 'C' + message.role + 'D' }}{% endfor %}";
    let messages = vec![
        ChatMessage { role: "x".to_string(), content: "".to_string() },
    ];
    let ctx = RenderContext::new();
    let rendered = render_chat_template_with_context(template, &messages, &ctx);
    assert_eq!(rendered, "ABCxD");
}

#[test]
fn nested_if_inside_for() {
    // if inside if (via elif chain)
    let template = r#"{% for message in messages %}{% if message.role == 'user' %}U{% elif message.role == 'system' %}S{% else %}O{% endif %}{% endfor %}"#;
    let messages = vec![
        ChatMessage { role: "user".to_string(), content: "".to_string() },
        ChatMessage { role: "system".to_string(), content: "".to_string() },
        ChatMessage { role: "tool".to_string(), content: "".to_string() },
    ];
    let ctx = RenderContext::new();
    let rendered = render_chat_template_with_context(template, &messages, &ctx);
    assert_eq!(rendered, "USO");
}

#[test]
fn special_characters_in_content() {
    let template = "{% for message in messages %}{{ message.content }}{% endfor %}";
    let messages = vec![
        ChatMessage { role: "user".to_string(), content: "Hello <world> & \"friends\"".to_string() },
    ];
    let ctx = RenderContext::new();
    let rendered = render_chat_template_with_context(template, &messages, &ctx);
    assert_eq!(rendered, "Hello <world> & \"friends\"");
}

#[test]
fn unicode_content() {
    let template = "{% for message in messages %}{{ message.content }}{% endfor %}";
    let messages = vec![
        ChatMessage { role: "user".to_string(), content: "こんにちは 🌍".to_string() },
    ];
    let ctx = RenderContext::new();
    let rendered = render_chat_template_with_context(template, &messages, &ctx);
    assert_eq!(rendered, "こんにちは 🌍");
}

#[test]
fn flag_default_false_when_missing() {
    // If add_generation_prompt is not in context at all, it should be falsy
    let template = "{% for message in messages %}{{ message.role }}{% if loop.last and add_generation_prompt %}PROMPT{% endif %}{% endfor %}";
    let messages = vec![
        ChatMessage { role: "user".to_string(), content: "".to_string() },
    ];
    let ctx = RenderContext::new(); // no flags set
    let rendered = render_chat_template_with_context(template, &messages, &ctx);
    assert_eq!(rendered, "user"); // no PROMPT appended
}

// ── Jinja2 comment stripping ──────────────────────────────────────────────

#[test]
fn comment_block_stripped() {
    // {# ... #} must not appear in output
    let template = "before{# this is a comment #}after";
    let messages: Vec<ChatMessage> = vec![];
    let ctx = RenderContext::new();
    let out = render_chat_template_with_context(template, &messages, &ctx);
    assert_eq!(out, "beforeafter");
}

#[test]
fn comment_block_with_trim_modifier_stripped() {
    // {#- ... #} — the trim-modifier variant used heavily in Llama 3.2 templates
    let template = "before{#- this is a comment #}after";
    let messages: Vec<ChatMessage> = vec![];
    let ctx = RenderContext::new();
    let out = render_chat_template_with_context(template, &messages, &ctx);
    assert!(!out.contains("{#"), "comment marker must not appear in output; got: {out:?}");
    assert!(!out.contains("#}"), "comment closer must not appear in output; got: {out:?}");
    assert!(!out.contains("this is a comment"), "comment body must be stripped; got: {out:?}");
}

#[test]
fn comment_between_tags_stripped() {
    // Realistic pattern from Llama 3.2: comment between two block tags
    let template = "{% set x = 1 %}{#- extract system message #}{{ x }}";
    let messages: Vec<ChatMessage> = vec![];
    let ctx = RenderContext::new();
    let out = render_chat_template_with_context(template, &messages, &ctx);
    assert_eq!(out, "1");
}

#[test]
fn comment_in_llama32_style_template_does_not_leak() {
    // Simplified version of the Llama 3.2 preamble that triggered the original bug.
    // The comment must vanish; content before/after must be preserved.
    let template = concat!(
        "{{- bos_token }}",
        "{#- This block extracts the system message, so we can slot it into the right place. #}",
        "{%- if messages[0]['role'] == 'system' %}",
        "{%- set system_message = messages[0]['content'] %}",
        "{%- endif %}",
        "{{- system_message }}",
    );
    let messages = vec![
        ChatMessage { role: "system".to_string(), content: "SYS".to_string() },
    ];
    let mut ctx = RenderContext::new();
    ctx.set_var("bos_token", "<BOS>");
    let out = render_chat_template_with_context(template, &messages, &ctx);
    assert!(!out.contains("{#"), "comment marker leaked into output: {out:?}");
    assert!(!out.contains("This block"), "comment body leaked into output: {out:?}");
    assert!(out.contains("<BOS>"), "bos_token missing from output: {out:?}");
    assert!(out.contains("SYS"), "system message not injected: {out:?}");
}

