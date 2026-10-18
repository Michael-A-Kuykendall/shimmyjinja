pub(crate) mod ast;
pub(crate) mod eval;
pub(crate) mod lexer;
pub(crate) mod parser;

use crate::eval::{Evaluator, Value};
use crate::parser::Parser;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Context variables available during template rendering.
///
/// These map to the top-level Jinja context that HF's
/// `tokenizer.apply_chat_template()` provides, such as `eos_token`,
/// `bos_token`, `add_generation_prompt`, etc.
#[derive(Debug, Clone, Default)]
pub struct RenderContext {
    /// String variables (e.g., "eos_token" -> "</s>", "bos_token" -> "<s>")
    pub vars: HashMap<String, String>,
    /// Boolean variables (e.g., "add_generation_prompt" -> true)
    pub flags: HashMap<String, bool>,
}

impl RenderContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a string variable in the context.
    pub fn set_var(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.vars.insert(key.into(), value.into());
        self
    }

    /// Set a boolean flag in the context.
    pub fn set_flag(&mut self, key: impl Into<String>, value: bool) -> &mut Self {
        self.flags.insert(key.into(), value);
        self
    }
}

/// Error type returned by the fallible render functions.
pub type RenderError = String;

/// Render a HF-style chat_template with messages and default context.
///
/// Default context: `eos_token = "</s>"`, `add_generation_prompt = true`.
/// For custom context use [`render_chat_template_with_context`].
/// For a fallible version that returns `Result`, use [`try_render_chat_template`].
///
/// # Panics
///
/// Panics if the template fails to parse or render. Use [`try_render_chat_template`]
/// if you need to handle template errors without unwinding.
pub fn render_chat_template(template: &str, messages: &[ChatMessage]) -> String {
    let mut ctx = RenderContext::new();
    ctx.set_var("eos_token", "</s>");
    ctx.set_flag("add_generation_prompt", true);
    render_chat_template_with_context(template, messages, &ctx)
}

/// Render a HF-style chat_template with messages and explicit context.
///
/// The context provides string variables (`eos_token`, `bos_token`) and
/// boolean flags (`add_generation_prompt`) that the template can reference.
///
/// # Panics
///
/// Panics if the template fails to parse or render. Use
/// [`try_render_chat_template_with_context`] for a non-panicking variant.
///
/// # Examples
///
/// ```
/// use shimmyjinja::{ChatMessage, RenderContext, render_chat_template_with_context};
///
/// let template = "{% for message in messages %}{{ message['role'] }}: {{ message['content'] }}\n{% endfor %}";
/// let messages = vec![ChatMessage { role: "user".into(), content: "Hello!".into() }];
/// let mut ctx = RenderContext::new();
/// ctx.set_var("eos_token", "</s>");
/// ctx.set_flag("add_generation_prompt", false);
///
/// let out = render_chat_template_with_context(template, &messages, &ctx);
/// assert_eq!(out, "user: Hello!\n");
/// ```
pub fn render_chat_template_with_context(
    template: &str,
    messages: &[ChatMessage],
    ctx: &RenderContext,
) -> String {
    try_render_chat_template_with_context(template, messages, ctx)
        .unwrap_or_else(|e| panic!("shimmyjinja render error: {}", e))
}

/// Render a HF-style chat_template, returning `Err` instead of panicking.
///
/// Default context: `eos_token = "</s>"`, `add_generation_prompt = true`.
pub fn try_render_chat_template(
    template: &str,
    messages: &[ChatMessage],
) -> Result<String, RenderError> {
    let mut ctx = RenderContext::new();
    ctx.set_var("eos_token", "</s>");
    ctx.set_flag("add_generation_prompt", true);
    try_render_chat_template_with_context(template, messages, &ctx)
}

/// Render a HF-style chat_template with explicit context, returning `Err` instead of panicking.
///
/// Prefer this over [`render_chat_template_with_context`] when you need to handle
/// template errors gracefully (e.g., fall back to a default format).
///
/// # Examples
///
/// ```
/// use shimmyjinja::{ChatMessage, RenderContext, try_render_chat_template_with_context};
///
/// let bad_template = "{% for x in %}oops{% endfor %}";
/// let messages = vec![ChatMessage { role: "user".into(), content: "hi".into() }];
/// let ctx = RenderContext::new();
///
/// assert!(try_render_chat_template_with_context(bad_template, &messages, &ctx).is_err());
/// ```
pub fn try_render_chat_template_with_context(
    template: &str,
    messages: &[ChatMessage],
    ctx: &RenderContext,
) -> Result<String, RenderError> {

    let mut parser = Parser::new(template);
    let ast = parser.parse().map_err(|e| format!("parse error: {}", e))?;

    let mut context = HashMap::new();

    // Transform messages into Value::Array of Value::Map
    let mut msgs_val = Vec::new();
    for m in messages {
        let mut map = HashMap::new();
        map.insert("role".to_string(), Value::String(m.role.clone()));
        map.insert("content".to_string(), Value::String(m.content.clone()));
        msgs_val.push(Value::Map(map));
    }
    context.insert("messages".to_string(), Value::Array(msgs_val));

    // Inject string variables from context
    for (k, v) in &ctx.vars {
        context.insert(k.clone(), Value::String(v.clone()));
    }

    // Inject boolean flags from context
    for (k, v) in &ctx.flags {
        context.insert(k.clone(), Value::Bool(*v));
    }

    let mut eval = Evaluator::new(context);
    eval.render(&ast).map_err(|e| format!("render error: {}", e))
}
