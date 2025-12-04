# shimmyjinja Design

## 1. Crate Purpose

`shimmyjinja` is a **minimal, LLM-focused Jinja-like engine** implemented in Rust. It exists to execute Hugging Face–style `chat_template` strings (including those embedded in GGUF metadata) against a list of chat messages and produce a final prompt string.

- **Input:**
  - A `chat_template` string (HF-style Jinja subset).
  - A list of `ChatMessage { role, content }`.
- **Output:**
  - A single `String` representing the fully rendered prompt.
- **Primary consumer:**
  - `libshimmy` and other Rust inference stacks that want to honor HF / GGUF `chat_template` semantics without Python or a full generic Jinja engine.

## 2. Non-goals

To keep scope tight and avoid fractal bloat, v0.x of `shimmyjinja` will **not**:

- Implement the full Jinja2 language (filters, macros, includes, custom tests, I/O, etc.).
- Serve as a generic web templating engine.
- Execute arbitrary user-provided functions or access the filesystem/network.
- Provide tokenization, stop-conditions, or model metadata logic (those live in `libshimmy`).

The crate is a **pure function**: `template + messages -> String`.

## 3. Public API (v0.1)

### 3.1 Core types

```rust
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}
```

This mirrors HF’s `messages` schema: each element has a `role` (e.g., `"system"`, `"user"`, `"assistant"`) and `content` (plain string for now).

### 3.2 Core function

```rust
pub fn render_chat_template(
    template: &str,
    messages: &[ChatMessage],
) -> String
```

- Deterministic and total: for any input string + message list, returns a string; no side effects.
- Panics only on programmer errors (e.g., impossible internal states), not on malformed templates; malformed constructs are rendered as literals where possible.
- All heavy lifting for model-specific behavior (special tokens, BOS/EOS, stops) is handled **outside** this crate.

## 4. Supported Template Subset (v0.1)

The goal is to support the **common subset** used by HF `chat_template`s for chat models like TinyLlama and Phi-3, while rejecting/ignoring features we don’t need.

### 4.1 Data model in templates

The template context will expose exactly one top-level variable:

- `messages`: a list of objects, where each `message` has:
  - `message.role: String`
  - `message.content: String`

No other globals are available in v0.1.

### 4.2 Control flow

Supported:

- A single-level `for` loop over `messages`:

  ```jinja
  {% for message in messages %}
  ...
  {% endfor %}
  ```

- Multiple `for message in messages` loops in the same template are allowed, but **no nested loops**.

Not supported in v0.1:

- Arbitrary loops over other collections.
- Nested `for` loops.

### 4.3 Expressions and variables

Supported variable expressions:

- `{{ message.role }}`
- `{{ message.content }}`

Any other expression inside `{{ ... }}` is treated as **literal text** in v0.1.

### 4.4 Conditionals (optional v0.1.1+)

Initial v0.1 implementation may skip conditionals entirely. If needed for real-world templates, we can add a tiny subset later:

- `{% if message.role == "system" %} ... {% endif %}`
- `{% if message.role == "user" %} ... {% endif %}`

Even then, only:

- Equality comparisons between `message.role` and a literal string.
- No `elif`, no `else`, no complex boolean expressions.

For now, templates requiring complex logic would either:

- Be simplified into this subset, or
- Be explicitly documented as unsupported.

### 4.5 Literals and newlines

- All other text (outside `{% ... %}` and `{{ ... }}`) is output as-is.
- Newlines are preserved from the template.

## 5. Implementation Strategy (v0.1)

### 5.1 Parser / interpreter shape

To keep things small and auditable, v0.1 will use a **very simple line-oriented interpreter**:

1. Split the template into lines.
2. Walk the lines in order, maintaining a small state machine:
   - `Top`: emitting lines directly.
   - `InForLoop`: collecting loop body lines until `{% endfor %}`.
3. On encountering `{% for message in messages %}`:
   - Switch to `InForLoop` and start collecting body lines.
4. On encountering `{% endfor %}`:
   - For each `ChatMessage` in `messages`, replay the collected body lines through the interpolation function and append to the output.
5. For lines outside loops:
   - Interpolate `{{ message.role }}` and `{{ message.content }}` against an **empty** `ChatMessage` (result is usually just literals; placeholders become empty or stay literal depending on behavior).

This is exactly what `src/lib.rs` currently sketches for v0.1.

### 5.2 Interpolation

- A tiny scanner looks for `{{ ... }}` and supports two keys:
  - `message.role`
  - `message.content`
- Unsupported keys are emitted as literal `{{ ... }}` blocks (fail-soft behavior).
- No filters (`|`) or function calls are parsed.

### 5.3 Error handling

- Unclosed `{{` or unmatched `{% for %}` / `{% endfor %}` are treated as best-effort literals.
- The API does **not** return `Result` in v0.1; errors are represented as degraded output, not panics.

## 6. Acceptance Corpus (Initial Targets)

`shimmyjinja` is driven by real `chat_template`s; to avoid drift, we pin a small corpus of concrete targets.

### 6.1 TinyLlama-1.1B-Chat-v1.0

- Source: HF `TinyLlama/TinyLlama-1.1B-Chat-v1.0` `tokenizer_config.json`.
- If it contains a `chat_template`, we will:
  - Copy the template string into tests (exactly, with a comment pointing to the HF commit/hash).
  - Validate that `render_chat_template` matches HF’s own rendering for a small set of `messages` examples.

### 6.2 Phi-3-mini-4k-instruct (or similar Phi-3 variant)

- Source: HF `microsoft/Phi-3-mini-4k-instruct` `tokenizer_config.json`.
- Same approach as TinyLlama: copy template into tests and compare behavior.

### 6.3 Synthetic minimal templates

- Simple templates used purely for unit tests, such as:

  ```jinja
  {% for message in messages %}{{ message.role }}: {{ message.content }}
  {% endfor %}
  ```

- These verify the core loop + interpolation behavior independent of any specific model.

## 7. Testing Strategy

1. **Unit tests** in `src/lib.rs` / `tests/basic.rs`:
   - Synthetic templates (simple `for` + interpolation).
   - Edge cases (no messages, empty template, malformed placeholders).

2. **Golden tests** (later, possibly in `tests/golden_*.rs`):
   - Real HF `chat_template` strings checked into the repo under `tests/data/`.
   - Hard-coded `Vec<ChatMessage>` examples.
   - Expected rendered strings stored alongside (or re-derived from a known-good Python/Jinja run outside of this crate).

## 8. Versioning and Future Extensions

- v0.1: current minimal subset (single `for` loop + `message.role`/`message.content`).
- v0.1.x: refine based on what TinyLlama / Phi-3 actually require (possibly adding tiny `if` support).
- v0.2+: consider:
  - Additional message fields (e.g., `tool_calls`), if HF templates start relying on them.
  - Optional support for a small, safe set of Jinja filters (e.g., `| trim`) if needed by real-world templates.

Any extension must be:

- Backwards compatible.
- Driven by a concrete `chat_template` from a real model.
- Covered by a golden test before being merged.
