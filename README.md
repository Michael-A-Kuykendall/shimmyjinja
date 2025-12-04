# shimmyjinja

Minimal Jinja-like engine for Hugging Face `chat_template` strings, written in Rust.

`shimmyjinja` exists to do one job well:

given a decoded HF-style `chat_template` string and a list of chat messages,
it produces the exact prompt string you expect – with explicit newline
semantics and a very small, auditable subset of Jinja.

## Status

- **Stage:** alpha
- **Scope:** narrowly focused on HF-style `chat_template` evaluation for
  LLM engines (e.g., libshimmy), not a general web templating engine.
- **Guarantee:** behavior is intentionally small, explicit, and driven by
  tests and design docs, not by surprise magic.

## Features

- Single-level `{% for message in messages %}...{% endfor %}` loops
  (any number of sequential loops; no nesting).
- Interpolation for:
  - `{{ message.role }}`
  - `{{ message.content }}`
- Explicit newline semantics:
  - No `"\n"` are ever invented by the engine.
  - All newlines come from the template string itself (after JSON decoding).
  - Trailing newlines are preserved; nothing is trimmed.
- Fail-soft behavior for malformed tags:
  - Missing `%}` in a `{% for ...` tag → the rest of the string is emitted
    literally.
  - Missing `{% endfor %}` → the `{% for ... %}` and its body are emitted
    literally.

All of this behavior is locked in by unit tests in `src/lib.rs` and described
in more detail in `SHIMMYJINJA_DESIGN.md`.

## Example

```rust
use shimmyjinja::{ChatMessage, render_chat_template};

fn main() {
    let template = "{% for message in messages %}{{ message.role }}: {{ message.content }}\n{% endfor %}";

    let messages = vec![
        ChatMessage { role: "system".into(), content: "You are a helpful assistant.".into() },
        ChatMessage { role: "user".into(), content: "Hello".into() },
    ];

    let rendered = render_chat_template(template, &messages);

    assert_eq!(rendered, "system: You are a helpful assistant.\nuser: Hello\n");
}
```

## Design & limitations

`shimmyjinja` is intentionally tiny:

- No nested loops.
- No `{% if %}` / `{% else %}`.
- No filters (e.g., `| upper`).
- No expressions beyond the literals and placeholders described above.

If you need a full Jinja implementation, you probably want a different
crate. If you need a predictable, HF-aligned engine that mirrors typical
LLM `chat_template` usage, you are in the right place.

For more detail, see `SHIMMYJINJA_DESIGN.md`.

## Governance & contributions

This crate is part of the broader **shimmy** ecosystem.

- The maintainer and project owner is **Michael Kuykendall**.
- Significant changes (especially to behavior that affects
  `chat_template` compatibility) should be discussed with the maintainer
  before opening a large PR.
- Small, focused improvements are welcome as pull requests, but the
  maintainer reserves the right to say "no" to changes that expand the
  scope beyond the intended narrow purpose.

If you are interested in becoming a regular contributor or co-maintainer,
please open an issue first so expectations and responsibilities can be
aligned up front.

## Support / sponsorship

If `shimmyjinja` or the wider shimmy tooling saved you time or production
bugs, and you want to say thanks:

- Consider sponsoring the maintainer on GitHub Sponsors, or
- Buy them a coffee via your preferred tipping platform.

Exact links and options live in the main shimmy project; this crate
follows the same "open source first, tip if you like it" philosophy.

## License

The license for this crate will match the main shimmy project.

Until that is finalized, treat this as **source-available for
experimentation and feedback**, not as a license grant for commercial
redistribution. When the license is formally chosen, this README and a
`LICENSE` file will be updated accordingly.
