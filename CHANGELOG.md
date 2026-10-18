# Changelog

All notable changes to this project will be documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [0.5.0] — 2026-05-31

### Added
- `try_render_chat_template` — non-panicking variant of `render_chat_template`,
  returns `Result<String, RenderError>`.
- `try_render_chat_template_with_context` — non-panicking variant with explicit
  `RenderContext`; the existing panicking functions now delegate to this.
- `RenderError` type alias (`pub type RenderError = String`).
- Doc-tests on the two primary public API functions.

### Changed
- Internal modules (`ast`, `eval`, `lexer`, `parser`) changed from `pub` to
  `pub(crate)`. They were never part of the documented API; this removes them
  from the public surface and resolves docs.rs noise. **Technically semver-breaking**
  for any downstream code importing `shimmyjinja::ast::*` etc. — but these modules
  were never documented or advertised as public API.

### Fixed
- `render_chat_template_with_context` now propagates error messages through the
  panic message (`shimmyjinja render error: <message>`) rather than a generic
  `"Template Parsing Error"` or `"Render Error"` string.

---

## [0.4.1] — 2026-05-28

### Fixed
- Strip `{#- ... #}` Jinja2 whitespace-trimming comment markers that appear in
  several Qwen3 and Phi-3.5 production templates.

---

## [0.4.0] — 2026-05-25

### Added
- Property-based test suite via `proptest` (13 properties): determinism, no
  content loss, no unresolved `{{`/`{%` tags, token literal pass-through,
  generation-prompt suppression, long-content stability, empty-input safety.
- `inline ternary` (`'yes' if flag else 'no'`).
- `namespace()` with `set ns.field = value` dotted assignment.
- `raise_exception()` support (raises `Err` during render).
- `message.get('key', 'default')` method call.
- `not in` membership test.

### Changed
- README rewritten with full feature table and model-family coverage list.
- Added `README_ZH.md` (Chinese README).

---

## [0.3.0] — 2026-05-20

### Added
- Full GGUF template support: `in` operator, `!=`, `<`, `>`, `<=`, `>=`.
- Real-model integration tests: 21 embedded-template math checks across TinyLlama,
  Llama 3.2, Mistral, Gemma 2, Phi-3.5, Qwen 2, Qwen 3, DeepSeek-LLM.
- GGUF end-to-end extraction tests (skip-if-missing, 9 tests).

---

## [0.2.1] — 2026-05-12

### Fixed
- Null-safety for missing template variables (render empty string, not panic).
- 12 additional edge-case tests.

---

## [0.2.0] — 2026-05-10

### Added
- AST-based engine replacing the previous regex approach.
- `if` / `elif` / `else` chains.
- String concatenation (`+`).
- `RenderContext` API for passing `eos_token`, `bos_token`, `add_generation_prompt`.

---

## [0.1.0] — 2026-05-05

Initial release: HF `chat_template` rendering, `for` loops, basic `if`, variable
substitution, TinyLlama template verified.
