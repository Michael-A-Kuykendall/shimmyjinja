use shimmyjinja::{render_chat_template, render_chat_template_with_context, ChatMessage, RenderContext};

#[test]
fn test_tinyllama_template_full_features() {
    let template = r#"
{% for message in messages %}
{% if message['role'] == 'user' %}
{{ '<|user|>\n' + message['content'] + eos_token }}
{% elif message['role'] == 'system' %}
{{ '<|system|>\n' + message['content'] + eos_token }}
{% elif message['role'] == 'assistant' %}
{{ '<|assistant|>\n'  + message['content'] + eos_token }}
{% endif %}
{% if loop.last and add_generation_prompt %}
{{ '<|assistant|>' }}
{% endif %}
{% endfor %}
"#
    .trim();

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: "You are a friendly AI.".to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: "Hello!".to_string(),
        },
    ];

    // Uses default context: eos_token="</s>", add_generation_prompt=true
    let rendered = render_chat_template(template, &messages);
    let expected = "<|system|>\nYou are a friendly AI.</s>\n<|user|>\nHello!</s>\n<|assistant|>";
    assert_eq!(rendered.trim(), expected);
}

#[test]
fn test_tinyllama_with_explicit_context() {
    let template = r#"
{% for message in messages %}
{% if message['role'] == 'user' %}
{{ '<|user|>\n' + message['content'] + eos_token }}
{% elif message['role'] == 'system' %}
{{ '<|system|>\n' + message['content'] + eos_token }}
{% elif message['role'] == 'assistant' %}
{{ '<|assistant|>\n'  + message['content'] + eos_token }}
{% endif %}
{% if loop.last and add_generation_prompt %}
{{ '<|assistant|>' }}
{% endif %}
{% endfor %}
"#
    .trim();

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: "You are a friendly AI.".to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: "Hello!".to_string(),
        },
    ];

    let mut ctx = RenderContext::new();
    ctx.set_var("eos_token", "</s>");
    ctx.set_flag("add_generation_prompt", true);

    let rendered = render_chat_template_with_context(template, &messages, &ctx);
    let expected = "<|system|>\nYou are a friendly AI.</s>\n<|user|>\nHello!</s>\n<|assistant|>";
    assert_eq!(rendered.trim(), expected);
}

#[test]
fn test_add_generation_prompt_false() {
    let template = r#"
{% for message in messages %}
{% if message['role'] == 'user' %}
{{ '<|user|>\n' + message['content'] + eos_token }}
{% endif %}
{% if loop.last and add_generation_prompt %}
{{ '<|assistant|>' }}
{% endif %}
{% endfor %}
"#
    .trim();

    let messages = vec![
        ChatMessage {
            role: "user".to_string(),
            content: "Hi".to_string(),
        },
    ];

    let mut ctx = RenderContext::new();
    ctx.set_var("eos_token", "</s>");
    ctx.set_flag("add_generation_prompt", false);

    let rendered = render_chat_template_with_context(template, &messages, &ctx);
    assert!(rendered.contains("<|user|>"));
    assert!(!rendered.contains("<|assistant|>"), "Should NOT contain assistant prompt when flag is false");
}

#[test]
fn test_custom_eos_token() {
    let template = r#"
{% for message in messages %}
{{ message['content'] + eos_token }}
{% endfor %}
"#
    .trim();

    let messages = vec![
        ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        },
    ];

    let mut ctx = RenderContext::new();
    ctx.set_var("eos_token", "<|endoftext|>");
    ctx.set_flag("add_generation_prompt", false);

    let rendered = render_chat_template_with_context(template, &messages, &ctx);
    assert!(rendered.contains("Hello<|endoftext|>"));
}

#[test]
fn test_multi_turn_conversation() {
    let template = r#"
{% for message in messages %}
{% if message['role'] == 'user' %}
{{ '<|user|>\n' + message['content'] + eos_token }}
{% elif message['role'] == 'system' %}
{{ '<|system|>\n' + message['content'] + eos_token }}
{% elif message['role'] == 'assistant' %}
{{ '<|assistant|>\n' + message['content'] + eos_token }}
{% endif %}
{% if loop.last and add_generation_prompt %}
{{ '<|assistant|>' }}
{% endif %}
{% endfor %}
"#
    .trim();

    let messages = vec![
        ChatMessage { role: "system".to_string(), content: "You help.".to_string() },
        ChatMessage { role: "user".to_string(), content: "What is 2+2?".to_string() },
        ChatMessage { role: "assistant".to_string(), content: "4".to_string() },
        ChatMessage { role: "user".to_string(), content: "Thanks!".to_string() },
    ];

    let mut ctx = RenderContext::new();
    ctx.set_var("eos_token", "</s>");
    ctx.set_flag("add_generation_prompt", true);

    let rendered = render_chat_template_with_context(template, &messages, &ctx);

    assert!(rendered.contains("<|system|>\nYou help.</s>"), "system msg");
    assert!(rendered.contains("<|user|>\nWhat is 2+2?</s>"), "user msg 1");
    assert!(rendered.contains("<|assistant|>\n4</s>"), "assistant msg");
    assert!(rendered.contains("<|user|>\nThanks!</s>"), "user msg 2");
    // Should end with generation prompt since it's the last message
    assert!(rendered.trim().ends_with("<|assistant|>"), "generation prompt at end");
}
