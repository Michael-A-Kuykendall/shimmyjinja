//! GGUF extraction tests.
//!
//! These tests read `tokenizer.chat_template` directly from real GGUF files,
//! render with a minimal set of test messages, and verify the output contains
//! the expected structural tokens.
//!
//! The tests **skip gracefully** when model files are not present, so CI
//! passes without any model downloads.
//!
//! Expected directory: `D:/shimmy-test-models/` (configurable via the
//! `SHIMMY_TEST_MODELS` environment variable).

use shimmyjinja::{render_chat_template_with_context, ChatMessage, RenderContext};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

// ── Minimal GGUF metadata reader ──────────────────────────────────────────

/// GGUF value types (subset — enough to skip over any field)
const GGUF_TYPE_UINT8:   u32 = 0;
const GGUF_TYPE_INT8:    u32 = 1;
const GGUF_TYPE_UINT16:  u32 = 2;
const GGUF_TYPE_INT16:   u32 = 3;
const GGUF_TYPE_UINT32:  u32 = 4;
const GGUF_TYPE_INT32:   u32 = 5;
const GGUF_TYPE_FLOAT32: u32 = 6;
const GGUF_TYPE_BOOL:    u32 = 7;
const GGUF_TYPE_STRING:  u32 = 8;
const GGUF_TYPE_ARRAY:   u32 = 9;
const GGUF_TYPE_UINT64:  u32 = 10;
const GGUF_TYPE_INT64:   u32 = 11;
const GGUF_TYPE_FLOAT64: u32 = 12;

fn read_u32_le<R: Read>(r: &mut R) -> std::io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn read_u64_le<R: Read>(r: &mut R) -> std::io::Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}

fn read_gguf_string<R: Read>(r: &mut R) -> std::io::Result<String> {
    let len = read_u64_le(r)? as usize;
    let mut bytes = vec![0u8; len];
    r.read_exact(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Skip over a single GGUF value without decoding it.
fn skip_gguf_value<R: Read>(r: &mut R, val_type: u32) -> std::io::Result<()> {
    match val_type {
        GGUF_TYPE_UINT8 | GGUF_TYPE_INT8 | GGUF_TYPE_BOOL => {
            let mut b = [0u8; 1];
            r.read_exact(&mut b)?;
        }
        GGUF_TYPE_UINT16 | GGUF_TYPE_INT16 => {
            let mut b = [0u8; 2];
            r.read_exact(&mut b)?;
        }
        GGUF_TYPE_UINT32 | GGUF_TYPE_INT32 | GGUF_TYPE_FLOAT32 => {
            read_u32_le(r)?;
        }
        GGUF_TYPE_UINT64 | GGUF_TYPE_INT64 | GGUF_TYPE_FLOAT64 => {
            read_u64_le(r)?;
        }
        GGUF_TYPE_STRING => {
            read_gguf_string(r)?;
        }
        GGUF_TYPE_ARRAY => {
            let elem_type = read_u32_le(r)?;
            let count = read_u64_le(r)?;
            for _ in 0..count {
                skip_gguf_value(r, elem_type)?;
            }
        }
        _ => {
            // Unknown type — cannot safely skip, propagate as an error
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unknown GGUF value type {}", val_type),
            ));
        }
    }
    Ok(())
}

/// Extract a string metadata field from a GGUF file.
/// Returns `None` if the file doesn't exist, isn't a valid GGUF, or the
/// key is not found.
fn extract_gguf_string_key(path: &Path, target_key: &str) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;

    // Magic
    let mut magic = [0u8; 4];
    f.read_exact(&mut magic).ok()?;
    if &magic != b"GGUF" {
        return None;
    }

    // Version
    let version = read_u32_le(&mut f).ok()?;

    // n_tensors, n_kv  (u64 for v2+, u32 for v1)
    let (_, n_kv) = if version >= 2 {
        let nt = read_u64_le(&mut f).ok()?;
        let nk = read_u64_le(&mut f).ok()?;
        (nt, nk)
    } else {
        let nt = read_u32_le(&mut f).ok()? as u64;
        let nk = read_u32_le(&mut f).ok()? as u64;
        (nt, nk)
    };

    for _ in 0..n_kv {
        let key = read_gguf_string(&mut f).ok()?;
        let val_type = read_u32_le(&mut f).ok()?;

        if val_type == GGUF_TYPE_STRING {
            let val = read_gguf_string(&mut f).ok()?;
            if key == target_key {
                return Some(val);
            }
        } else {
            skip_gguf_value(&mut f, val_type).ok()?;
        }
    }

    None
}

/// Convenience: extract `tokenizer.chat_template` from a GGUF file.
fn read_chat_template(path: &Path) -> Option<String> {
    extract_gguf_string_key(path, "tokenizer.chat_template")
}

// ── Test helpers ───────────────────────────────────────────────────────────

fn model_dir() -> PathBuf {
    let base = std::env::var("SHIMMY_TEST_MODELS")
        .unwrap_or_else(|_| "D:/shimmy-test-models".to_string());
    PathBuf::from(base)
}

fn two_turn_messages() -> Vec<ChatMessage> {
    vec![
        ChatMessage { role: "user".into(),      content: "What is 2+2?".into() },
        ChatMessage { role: "assistant".into(),  content: "4".into() },
        ChatMessage { role: "user".into(),       content: "And 4+4?".into() },
    ]
}

fn default_ctx() -> RenderContext {
    let mut c = RenderContext::new();
    c.set_var("bos_token",  "<s>");
    c.set_var("eos_token",  "</s>");
    c.set_flag("add_generation_prompt", true);
    c
}

macro_rules! skip_if_missing {
    ($path:expr) => {
        if !$path.exists() {
            eprintln!("SKIP — model not found: {}", $path.display());
            return;
        }
    };
}

// ── GGUF extraction tests ─────────────────────────────────────────────────

#[test]
fn gguf_llama32_1b_chat_template_renders() {
    let path = model_dir().join("gguf_collection/Llama-3.2-1B-Instruct-Q4_K_M.gguf");
    skip_if_missing!(path);

    let template = read_chat_template(&path)
        .expect("tokenizer.chat_template not found in Llama-3.2-1B GGUF");

    let mut ctx = RenderContext::new();
    ctx.set_var("bos_token",  "<|begin_of_text|>");
    ctx.set_var("eos_token",  "<|end_of_text|>");
    ctx.set_flag("add_generation_prompt", true);

    let rendered = render_chat_template_with_context(&template, &two_turn_messages(), &ctx);

    assert!(!rendered.is_empty(), "rendered output is empty");
    // Llama 3 uses these structural tokens
    assert!(
        rendered.contains("<|start_header_id|>"),
        "Llama3 structural token missing:\n{}", rendered
    );
    assert!(
        rendered.contains("What is 2+2?"),
        "user content missing:\n{}", rendered
    );
}

#[test]
fn gguf_qwen25_chat_template_renders() {
    let path = model_dir().join("gguf_collection/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf");
    skip_if_missing!(path);

    let template = read_chat_template(&path)
        .expect("tokenizer.chat_template not found in Qwen2.5 GGUF");

    let rendered = render_chat_template_with_context(&template, &two_turn_messages(), &default_ctx());

    assert!(!rendered.is_empty());
    // ChatML structural tokens
    assert!(
        rendered.contains("<|im_start|>"),
        "ChatML im_start missing:\n{}", rendered
    );
    assert!(
        rendered.contains("<|im_end|>"),
        "ChatML im_end missing:\n{}", rendered
    );
    assert!(
        rendered.contains("What is 2+2?"),
        "user content missing:\n{}", rendered
    );
}

#[test]
fn gguf_gemma2_chat_template_renders() {
    let path = model_dir().join("gguf_collection/gemma-2-2b-it-Q4_K_M.gguf");
    skip_if_missing!(path);

    let template = read_chat_template(&path)
        .expect("tokenizer.chat_template not found in Gemma2 GGUF");

    let mut ctx = RenderContext::new();
    ctx.set_var("bos_token", "<bos>");
    ctx.set_var("eos_token", "<eos>");
    ctx.set_flag("add_generation_prompt", true);

    let rendered = render_chat_template_with_context(&template, &two_turn_messages(), &ctx);

    assert!(!rendered.is_empty());
    assert!(
        rendered.contains("<start_of_turn>"),
        "Gemma start_of_turn missing:\n{}", rendered
    );
    assert!(
        rendered.contains("What is 2+2?"),
        "user content missing:\n{}", rendered
    );
}

#[test]
fn gguf_tinyllama_chat_template_renders() {
    let path = model_dir().join("gguf_collection/TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf");
    skip_if_missing!(path);

    let template = read_chat_template(&path)
        .expect("tokenizer.chat_template not found in TinyLlama GGUF");

    let mut ctx = RenderContext::new();
    ctx.set_var("eos_token", "</s>");
    ctx.set_flag("add_generation_prompt", true);

    let rendered = render_chat_template_with_context(&template, &two_turn_messages(), &ctx);

    assert!(!rendered.is_empty());
    assert!(
        rendered.contains("<|user|>") || rendered.contains("[INST]"),
        "TinyLlama user token missing:\n{}", rendered
    );
    assert!(
        rendered.contains("What is 2+2?"),
        "user content missing:\n{}", rendered
    );
}

/// Generic test: any GGUF that has a chat_template should render without panic
/// and produce non-empty output.
#[test]
fn gguf_any_available_model_renders() {
    let dir = model_dir().join("gguf_collection");
    if !dir.exists() {
        eprintln!("SKIP — gguf_collection directory not found at {}", dir.display());
        return;
    }

    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("SKIP — cannot read {}: {}", dir.display(), e);
            return;
        }
    };

    let mut tested = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "gguf" {
            continue;
        }

        let template = match read_chat_template(&path) {
            Some(t) => t,
            None => {
                eprintln!("  no chat_template in {}", path.display());
                continue;
            }
        };

        let rendered = render_chat_template_with_context(
            &template, &two_turn_messages(), &default_ctx(),
        );
        assert!(
            !rendered.is_empty(),
            "empty render for {}",
            path.display()
        );
        eprintln!("  OK  {} ({} bytes rendered)", path.file_name().unwrap().to_string_lossy(), rendered.len());
        tested += 1;
    }

    eprintln!("Tested {} GGUF files", tested);
}
