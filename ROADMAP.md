# shimmyjinja Roadmap

This document sketches potential future improvements for `shimmyjinja`.
None of these are promises; they are ideas to evaluate as real needs arise.

## v0.1.x – Hardening and parity

- Add golden tests using real Hugging Face `chat_template` strings from:
  - TinyLlama chat models
  - Phi-3 instruct models
- Expand test coverage for:
  - Whitespace around tags (`{%  for ... %}` vs `{% for ... %}`)
  - Mixed literal / tag content on the same line
  - Very long message lists and templates
- Validate behavior against llama.cpp / other engines at the string level.

## v0.2 – Optional strict mode

- Introduce a `try_render_chat_template(...) -> Result<String, TemplateError>` API
  alongside the existing `render_chat_template`, allowing callers to:
  - Distinguish between successful renders and clearly malformed templates.
  - Opt into stricter error reporting instead of always emitting literals.

## v0.3+ – Carefully scoped feature expansion

Only to be considered if real-world HF templates require it, and *not* before:

- Limited `{% if message.role == ... %}` support for a few common patterns.
- Simple filters (e.g., `| trim`) if they show up in widely-used templates.
- Better diagnostics / tracing hooks for debugging prompt construction.

## Non-goals (for now)

- Becoming a general-purpose Jinja engine.
- Supporting arbitrary expressions, complex filters, or nested control flow.
- Handling I/O, file inclusion, or any side effects.

All changes should preserve the crate’s core values:

- Small, auditable implementation.
- Explicit newline and control-flow semantics.
- Behavior locked in by tests and real-world templates.
