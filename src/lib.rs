//! shimmyjinja: minimal Jinja-like engine for HF-style `chat_template` strings.
//!
//! This crate exists to do one job well: evaluate a decoded Hugging Face
//! `chat_template` string against a list of messages and return the
//! rendered prompt, with explicit newline semantics and a very small,
//! auditable subset of Jinja.
//!
//! Supported subset:
//! - Literals.
//! - `{{ message.role }}` and `{{ message.content }}`.
//! - `{% for message in messages %} ... {% endfor %}` (single-level, any
//!   number of sequential loops).
//!
//! Not supported:
//! - Nested loops.
//! - `{% if %}` / `{% else %}` blocks.
//! - Filters (e.g., `| upper`).
//! - Arbitrary expressions or side effects.
//!
//! Newline semantics:
//! - Newlines are only those present in the `chat_template` string
//!   itself (after JSON decoding).
//! - The engine never injects extra `\n` characters.
//! - Trailing newlines are preserved; no trimming is performed.
//!
//! For more background and design notes, see `SHIMMYJINJA_DESIGN.md` in
//! the repository.

/// A single chat message in HF-style templates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Render a minimal HF-style chat_template with a list of messages.
///
/// Supported subset (by design, very small):
/// - Literals.
/// - `{{ message.role }}` and `{{ message.content }}`.
/// - `{% for message in messages %} ... {% endfor %}`.
/// Nested loops, filters, and arbitrary expressions are not supported.
pub fn render_chat_template(template: &str, messages: &[ChatMessage]) -> String {
    // Extremely small substring-based interpreter for a single, flat
    // `{% for message in messages %} ... {% endfor %}` loop plus literals.
    // Newlines are *only* those present in `template` itself; we never
    // inject `\n`.

    let mut output = String::new();
    let mut rest = template;

    loop {
        // Look for the next for-tag.
        if let Some(for_start) = rest.find("{% for") {
            // Emit everything before the for-tag as a literal, with
            // placeholders rendered against an empty message.
            let (before, after_for) = rest.split_at(for_start);
            if !before.is_empty() {
                output.push_str(&render_line(before, &ChatMessage {
                    role: String::new(),
                    content: String::new(),
                }));
            }

            // Find the end of the for-tag.
            let for_close = match after_for.find("%}") {
                Some(idx) => idx + 2,
                None => {
                    // Malformed tag (no closing `%}`): emit the rest
                    // literally and stop to avoid panicking.
                    output.push_str(after_for);
                    break;
                }
            };

            let (_for_tag, after_for_tag) = after_for.split_at(for_close);

            // Within the remaining string, find the matching `{% endfor %}`.
            let end_tag_start = match after_for_tag.find("{% endfor %}") {
                Some(idx) => idx,
                None => {
                    // No end tag; treat the rest (including the for-tag
                    // we just saw) as literal. This is our "best-effort"
                    // behavior for malformed loops.
                    output.push_str(after_for);
                    break;
                }
            };

            let (body, after_body_and_end) = after_for_tag.split_at(end_tag_start);

            // Skip over the `{% endfor %}` marker itself.
            let end_tag_len = "{% endfor %}".len();
            let after_end = if after_body_and_end.len() >= end_tag_len {
                &after_body_and_end[end_tag_len..]
            } else {
                ""
            };

            // Replay the body for each message.
            for msg in messages {
                output.push_str(&render_line(body, msg));
            }

            // Continue scanning after this loop; this naturally supports
            // multiple *sequential* loops and any literal text that
            // follows.
            rest = after_end;
        } else {
            // No more for-loops; render the remainder as a literal.
            if !rest.is_empty() {
                output.push_str(&render_line(rest, &ChatMessage {
                    role: String::new(),
                    content: String::new(),
                }));
            }
            break;
        }
    }

    output
}

fn render_line(line: &str, msg: &ChatMessage) -> String {
    // Extremely small interpolation engine for `{{ message.role }}` and `{{ message.content }}`.
    let mut out = String::with_capacity(line.len() + 16);
    let mut rest = line;

    while let Some(start) = rest.find("{{") {
        let (before, after_start) = rest.split_at(start);
        out.push_str(before);

        if let Some(end) = after_start.find("}}"){ 
            let (placeholder, after_end) = after_start.split_at(end + 2);
            let key = placeholder.trim_start_matches("{{").trim_end_matches("}}").trim();

            match key {
                "message.role" => out.push_str(&msg.role),
                "message.content" => out.push_str(&msg.content),
                _ => {
                    // Unsupported placeholder: emit as-is.
                    out.push_str(placeholder);
                }
            }

            rest = after_end;
        } else {
            // Unclosed placeholder; emit the rest as-is.
            out.push_str(after_start);
            rest = "";
        }
    }

    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_for_loop_over_messages() {
        // HF-style after JSON decoding: single-line template with actual
        // newline characters in the body.
        let template = "{% for message in messages %}{{ message.role }}: {{ message.content }}\n{% endfor %}";

        let messages = vec![
            ChatMessage { role: "system".into(), content: "You are a helpful assistant.".into() },
            ChatMessage { role: "user".into(), content: "Hello".into() },
        ];

        let rendered = render_chat_template(template, &messages);
        // Newlines now come solely from the template; the final string
        // includes the explicit `\\n` from the template body.
        let expected = "system: You are a helpful assistant.\nuser: Hello\n";

        assert_eq!(rendered, expected);
    }

    #[test]
    fn for_loop_with_no_newlines_in_template() {
        // Template has no `\n` at all; engine must not invent any.
        let template = "{% for message in messages %}{{ message.role }}: {{ message.content }}{% endfor %}";

        let messages = vec![
            ChatMessage { role: "system".into(), content: "You are a helpful assistant.".into() },
            ChatMessage { role: "user".into(), content: "Hello".into() },
        ];

        let rendered = render_chat_template(template, &messages);
        let expected = "system: You are a helpful assistant.user: Hello";

        assert_eq!(rendered, expected);
    }

    #[test]
    fn multiple_sequential_loops_and_literals() {
        let template = "prefix-\n\
{% for message in messages %}A: {{ message.role }}\n{% endfor %}\
middle-\n\
{% for message in messages %}B: {{ message.content }}\n{% endfor %}suffix";

        let messages = vec![
            ChatMessage { role: "system".into(), content: "You are a helpful assistant.".into() },
            ChatMessage { role: "user".into(), content: "Hello".into() },
        ];

        let rendered = render_chat_template(template, &messages);
        let expected = concat!(
            "prefix-\n",
            "A: system\n",
            "A: user\n",
            "middle-\n",
            "B: You are a helpful assistant.\n",
            "B: Hello\n",
            "suffix",
        );

        assert_eq!(rendered, expected);
    }

    #[test]
    fn malformed_for_missing_endfor_is_literal() {
        let template = "before {% for message in messages %}broken";

        let messages = vec![ChatMessage { role: "system".into(), content: "x".into() }];

        let rendered = render_chat_template(template, &messages);
        let expected = "before {% for message in messages %}broken";

        assert_eq!(rendered, expected);
    }

    #[test]
    fn malformed_for_missing_closing_percent_brace_is_literal() {
        let template = "oops {% for message in messages broken {% endfor %} tail";

        let messages = vec![ChatMessage { role: "user".into(), content: "y".into() }];

        let rendered = render_chat_template(template, &messages);
        let expected = "oops {% for message in messages broken {% endfor %} tail";

        assert_eq!(rendered, expected);
    }
}
