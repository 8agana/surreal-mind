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

---

## Session 11 Debugging (2026-01-07)

### What Was Tried

1. **vision-plain architecture** (correct approach per mistral.rs docs)
   - Command: `mistralrs-server --port 8111 vision-plain -m mistralai/Ministral-3-3B-Instruct-2512`
   - Result: Server starts, model loads, but inference fails with `Metal strided to_dtype F8E4M3 F32 not implemented`
   - Root cause: Model uses FP8 operations that Metal backend doesn't support

2. **Config.json fixes**
   - Added flat `rope_theta` to text_config (1000000.0)
   - Added flat `rope_theta` to vision_config (10000.0)
   - This fixed the "missing field rope_theta" parse error

3. **ISQ Q4K quantization**
   - Command: `mistralrs-server --port 8111 --isq Q4K vision-plain -m mistralai/Ministral-3-3B-Instruct-2512`
   - Result: Server starts, dummy run completes WITHOUT FP8 error
   - But inference produces garbage output (random multilingual tokens)
   - Observation: 541 prompt tokens first request, 22 tokens second request - suggests tokenization/template issue

4. **GGUF attempt**
   - Command: `mistralrs-server --port 8111 gguf -t mistralai/Ministral-3-3B-Instruct-2512 -m mistralai/Ministral-3-3B-Instruct-2512-GGUF -f Ministral-3-3B-Instruct-2512-Q4_K_M.gguf`
   - Result: Panic with `Unknown GGUF architecture 'mistral3'`

### Key Findings

- **vision-plain is correct architecture** (not plain -a mistral) - confirmed via mistral.rs MISTRAL3.md docs
- **ISQ bypasses FP8 error** but something else is broken (tokenization or template)
- **Token count discrepancy** (541 vs 22) suggests chat template not applying correctly
- **GGUF loader doesn't recognize mistral3** architecture yet

### Suspected Root Cause

Chat template requirements not satisfied. The ISQ version runs inference but produces garbage, which points to prompt construction issue rather than model capability.

### Next Steps

- Investigate mistral.rs chat template handling for Mistral 3
- Check if custom jinja template needed (like Mistral Small 3.1 requires)
- Debug why token counts differ between requests
- Consider if tokenizer.json from base model vs instruct model matters

---

## Session 12 Debugging (2026-01-07)

### What Was Tried

1. **Explicit chat template override**
   - Command: `mistralrs-server --port 8111 --isq Q4K -j <chat_template.jinja> vision-plain -m mistralai/Ministral-3-3B-Instruct-2512`
   - Result: Still garbage output (nonsense tokens), so not just a missing template.

2. **CPU-only run (no Metal)**
   - Command: `mistralrs-server --cpu -j <chat_template.jinja> vision-plain -m mistralai/Ministral-3-3B-Instruct-2512`
   - Result: No Metal FP8 error, but output is still garbage.

3. **Updated mistralrs-server to latest git**
   - Command: `cargo install --git https://github.com/EricLBuehler/mistral.rs --features metal,accelerate mistralrs-server`
   - Result: Still fails with Metal FP8 error; no behavior change.

4. **Offline dequantization attempts (convert FP8 -> BF16)**
   - Confirmed safetensors contain **F8_E4M3** weights plus **BF16** `weight_scale_inv` and `activation_scale` tensors.
   - Built converted model under `~/.cache/huggingface/converted/Ministral-3-3B-Instruct-2512-f16` and tried multiple scaling schemes:
     - `f8 * weight_scale_inv`
     - `f8 * weight_scale_inv * activation_scale`
     - `f8 / weight_scale_inv`
   - Result: Model loads from converted weights but still produces garbage output.
   - Converted model directory was later deleted per cleanup request.

### Key Findings

- **Root cause is likely FP8 scaling not implemented correctly for Mistral 3** in mistralrs.
- The model uses **scalar per-layer scales** (BF16) that mistralrs’ FP8 paths do not appear to consume.
- `activation_scale` tensors exist but are not referenced in mistralrs codebase.
- Metal backend cannot handle FP8 (`F8E4M3`) for this model (`Metal strided to_dtype F8E4M3 F32 not implemented`).
- CPU runs complete but output is nonsense, which aligns with FP8 weights being interpreted without proper scaling.

### Notes

- A temporary venv was created at `surreal-mind/.venv-convert` with numpy for conversion scripts.
- Converted model path (now removed): `~/.cache/huggingface/converted/Ministral-3-3B-Instruct-2512-f16/`.

### Current Blocker

**mistralrs lacks Mistral 3 FP8 dequant support (and likely scale handling),** so both Metal and CPU produce unusable outputs.

### Suggested Next Steps

- Patch mistralrs to apply Mistral 3’s FP8 `weight_scale_inv` (and possibly `activation_scale`) during load/dequant.
- Look for an official BF16/FP16 (non-FP8) checkpoint of `Ministral-3-3B-Instruct-2512`.
- As fallback, run this model via another backend that supports Mistral 3 FP8 (if acceptable).

## Reference task-33 for the installation of DeepSeek-Coder-V2-Lite-Instruct
