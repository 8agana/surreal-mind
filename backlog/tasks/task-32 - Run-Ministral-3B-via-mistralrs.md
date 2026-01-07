# Task 32: Run Ministral-3B via mistralrs-server

## Objective
Get `mistralai/Ministral-3-3B-Instruct-2512` running locally via `mistralrs-server` as the Scalpel backend.

**Model is non-negotiable.** This was a consensus decision.

---

## Current State

### What Works
- [x] `mistralrs-server` installed (`cargo install --git ... --features metal,accelerate`)
- [x] Model downloaded to `~/.cache/huggingface/hub/models--mistralai--Ministral-3-3B-Instruct-2512/`
- [x] HF token configured at `~/.cache/huggingface/token`
- [x] Scalpel tool code fully implemented in `surreal-mind/src/tools/scalpel.rs`

### Where It's Stuck
**Error:** `cannot find tensor model.embed_tokens.weight`

**Root Cause:** The `Ministral-3-3B-Instruct-2512` model uses a VLM-style tensor naming convention:
```
language_model.model.embed_tokens.weight
language_model.model.layers.0.input_layernorm.weight
...
```

But `mistralrs` with `-a mistral` expects standard naming:
```
model.embed_tokens.weight
model.layers.0.input_layernorm.weight
...
```

This mismatch causes the loader to fail.

---

## Work Already Done (Antigravity)

1. **Identified correct model ID:** `mistralai/Ministral-3-3B-Instruct-2512` (not the 8B variant)
2. **Resolved port conflict:** Changed from 8080 to 8111
3. **Resolved HF auth:** Wrote token to `~/.cache/huggingface/token`
4. **Patched config.json:** Removed `quantization_config` block that was causing parse errors
5. **Flattened config.json:** Moved `vocab_size` etc. to root level (still didn't help)
6. **Inspected safetensors:** Confirmed `language_model.model.*` prefix in weights

**Command that was attempted:**
```bash
mistralrs-server --port 8111 --isq Q4K plain -m mistralai/Ministral-3-3B-Instruct-2512 -a mistral
```

---

## Proposed Solutions (Pick One)

### Option A: Use GGUF Version
Mistral provides a GGUF version: `mistralai/Ministral-3-3B-Instruct-2512-GGUF`

Try:
```bash
mistralrs-server --port 8111 gguf -m mistralai/Ministral-3-3B-Instruct-2512-GGUF -f <filename>.gguf
```

Need to check: Does the GGUF loader handle the prefix differently?

### Option B: Use llama.cpp Instead
`llama.cpp` may have better support for newer Mistral architectures.

```bash
brew install llama.cpp
llama-server -m ~/.cache/huggingface/hub/.../model.safetensors --port 8111
```

Would need to verify API compatibility with Scalpel's OpenAI-style requests.

### Option C: Use vLLM
vLLM has explicit Mistral 3 support.

```bash
pip install vllm
python -m vllm.entrypoints.openai.api_server --model mistralai/Ministral-3-3B-Instruct-2512 --port 8111
```

Downside: Python dependency, higher memory overhead.

### Option D: Patch mistralrs
File an issue or PR to `mistral.rs` to add `language_model.` prefix stripping for Mistral 3 models.

---

## Files to Reference

| File | Purpose |
|------|---------|
| `src/tools/scalpel.rs` | Tool handler (done) |
| `src/clients/local.rs` | HTTP client to mistralrs (done) |
| `~/.cache/huggingface/hub/models--mistralai--Ministral-3-3B-Instruct-2512/` | Downloaded model |

---

## Acceptance Criteria

1. `mistralrs-server` (or alternative) runs on port 8111
2. Model loaded is `Ministral-3-3B-Instruct-2512`
3. `curl http://localhost:8111/v1/chat/completions` returns valid response
4. Scalpel tool works end-to-end via surreal-mind
