# Task 38: Swap Scalpel to Hermes 3 (Llama 3.2 3B)

## Status: COMPLETED
**Reason:** Ministral 3B failed (unsupported arch). Qwen 2.5 was skipped in favor of Hermes 3 (based on Llama 3.2) per user request.
**Goal:** Deploy `NousResearch/Hermes-3-Llama-3.2-3B-GGUF` as the stable backend for Scalpel.

## Implementation Steps
- [x] Update `local.rs` default model to `NousResearch/Hermes-3-Llama-3.2-3B-GGUF`
- [x] Update `scalpel.rs` agent instance metadata
- [x] Verify server launch with: `mistralrs-server --port 8111 gguf -m NousResearch/Hermes-3-Llama-3.2-3B-GGUF -f Hermes-3-Llama-3.2-3B.Q4_K_M.gguf`
- [x] Run inference smoke test
