pub mod ast;
pub mod eval;
pub mod lexer;
pub mod parser;

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

/// Render a HF-style chat_template with messages and default context.
///
/// Default context: `eos_token = "</s>"`, `add_generation_prompt = true`.
/// For custom context, use [`render_chat_template_with_context`].
///
/// Supported subset of Jinja2:
/// - Loops: `{% for message in messages %}`
/// - Conditions: `{% if ... %}`, `{% elif ... %}`, `{% else %}`
/// - Variables: `{{ message.role }}`, `{{ message['content'] }}`
/// - Literals: Strings, Booleans
/// - Operators: `==`, `+` (string concat), `and`, `or`
/// - Context: `messages` (provided), plus any variables from `RenderContext`
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
pub fn render_chat_template_with_context(
    template: &str,
    messages: &[ChatMessage],
    ctx: &RenderContext,
) -> String {
    let mut parser = Parser::new(template);
    let ast = match parser.parse() {
        Ok(ast) => ast,
        Err(e) => panic!("Template Parsing Error: {}", e),
    };

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
    match eval.render(&ast) {
        Ok(s) => s,
        Err(e) => panic!("Render Error: {}", e),
    }
}
