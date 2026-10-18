//! Built-in language model runner: quantized Qwen2.5 (GGUF) on the CPU via candle.
//!
//! Pure Rust, so it links cleanly next to whisper.cpp and needs no external server.
//! The fixed instruction part of the prompt is evaluated once and its KV cache reused,
//! so each request only pays for the user's own words.

use crate::qwen2::ModelWeights;
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::Instant;
use tokenizers::Tokenizer;

struct Loaded {
    weights_path: PathBuf,
    model: ModelWeights,
    tokenizer: Tokenizer,
    stop_tokens: Vec<u32>,
    /// Prompt prefix and a model copy whose KV cache already holds it.
    prefix: Option<(String, ModelWeights, usize)>,
}

/// Loaded model, kept between requests so only the first request pays the load cost.
static MODEL: LazyLock<Mutex<Option<Loaded>>> = LazyLock::new(|| Mutex::new(None));

fn err(e: candle_core::Error) -> String {
    format!("Language model error: {e}")
}

fn load(weights: &Path, tokenizer: &Path) -> Result<Loaded, String> {
    let mut file =
        std::fs::File::open(weights).map_err(|e| format!("Cannot open language model: {e}"))?;
    let content = gguf_file::Content::read(&mut file)
        .map_err(|e| format!("Language model file is invalid: {e}"))?;
    let model = ModelWeights::from_gguf(content, &mut file, &Device::Cpu)
        .map_err(|e| format!("Failed to load language model: {e}"))?;
    let tokenizer =
        Tokenizer::from_file(tokenizer).map_err(|e| format!("Failed to load tokenizer: {e}"))?;
    let stop_tokens = ["<|im_end|>", "<|endoftext|>"]
        .iter()
        .filter_map(|t| tokenizer.token_to_id(t))
        .collect();
    Ok(Loaded {
        weights_path: weights.to_path_buf(),
        model,
        tokenizer,
        stop_tokens,
        prefix: None,
    })
}

fn encode(tokenizer: &Tokenizer, text: &str) -> Result<Vec<u32>, String> {
    Ok(tokenizer
        .encode(text, false)
        .map_err(|e| format!("Tokenizer error: {e}"))?
        .get_ids()
        .to_vec())
}

fn forward(model: &mut ModelWeights, tokens: &[u32], pos: usize) -> Result<Tensor, String> {
    let input = Tensor::new(tokens, &Device::Cpu)
        .and_then(|t| t.unsqueeze(0))
        .map_err(err)?;
    model.forward(&input, pos).and_then(|l| l.squeeze(0)).map_err(err)
}

/// Load the model and evaluate `prefix` ahead of time, so the first real request is fast.
pub fn warm_up(weights: &Path, tokenizer: &Path, prefix: &str) -> Result<(), String> {
    let mut guard = MODEL.lock().map_err(|_| "Language model is unavailable")?;
    ensure_prefix(&mut guard, weights, tokenizer, prefix).map(|_| ())
}

fn ensure_prefix<'a>(
    slot: &'a mut Option<Loaded>,
    weights: &Path,
    tokenizer: &Path,
    prefix: &str,
) -> Result<&'a mut Loaded, String> {
    if slot.as_ref().map(|l| l.weights_path.as_path()) != Some(weights) {
        let started = Instant::now();
        *slot = None;
        *slot = Some(load(weights, tokenizer)?);
        eprintln!("🧠 Loaded language model in {:.1}s", started.elapsed().as_secs_f32());
    }
    let loaded = slot.as_mut().unwrap();
    if loaded.prefix.as_ref().map(|(p, ..)| p.as_str()) != Some(prefix) {
        let started = Instant::now();
        let tokens = encode(&loaded.tokenizer, prefix)?;
        let mut model = loaded.model.clone();
        model.clear_kv_cache();
        forward(&mut model, &tokens, 0)?;
        eprintln!("🧠 Cached {}-token prompt prefix in {:.1}s", tokens.len(), started.elapsed().as_secs_f32());
        loaded.prefix = Some((prefix.to_string(), model, tokens.len()));
    }
    Ok(loaded)
}

/// Complete `prefix + suffix` and return the generated text.
/// `prefix` should be the fixed part of the prompt; it is evaluated once and cached.
/// Generation stops early once `is_complete` reports the reply is usable.
pub fn complete(
    weights: &Path,
    tokenizer: &Path,
    prefix: &str,
    suffix: &str,
    max_tokens: usize,
    is_complete: impl Fn(&str) -> bool,
) -> Result<String, String> {
    let mut guard = MODEL.lock().map_err(|_| "Language model is unavailable")?;
    let loaded = ensure_prefix(&mut guard, weights, tokenizer, prefix)?;
    let started = Instant::now();

    let (_, prefilled, prefix_len) = loaded.prefix.as_ref().unwrap();
    let mut model = prefilled.clone();
    let mut pos = *prefix_len;
    let suffix_tokens = encode(&loaded.tokenizer, suffix)?;
    let logits = forward(&mut model, &suffix_tokens, pos)?;
    pos += suffix_tokens.len();

    let mut sampler = LogitsProcessor::from_sampling(0, Sampling::ArgMax);
    let mut next = sampler.sample(&logits).map_err(err)?;
    let mut generated: Vec<u32> = Vec::new();
    for i in 0..max_tokens {
        if loaded.stop_tokens.contains(&next) {
            break;
        }
        generated.push(next);
        if i % 4 == 0 {
            let text = loaded.tokenizer.decode(&generated, true).unwrap_or_default();
            if is_complete(&text) {
                break;
            }
        }
        let logits = forward(&mut model, &[next], pos)?;
        pos += 1;
        next = sampler.sample(&logits).map_err(err)?;
    }

    eprintln!(
        "🧠 {} prompt + {} generated tokens in {:.1}s",
        suffix_tokens.len(),
        generated.len(),
        started.elapsed().as_secs_f32()
    );
    loaded
        .tokenizer
        .decode(&generated, true)
        .map_err(|e| format!("Tokenizer error: {e}"))
}
