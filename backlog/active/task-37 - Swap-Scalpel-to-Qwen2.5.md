# Task 37: Swap Scalpel to Qwen 2.5 3B

## Status: ACTIVE
**Reason:** Ministral 3B failed on Mac (Metal panic on F32, "Unknown architecture" on GGUF).
**Goal:** Deploy `Qwen/Qwen2.5-3B-Instruct` as the stable backend for Scalpel.

## Implementation Steps
- [ ] Update `local.rs` default model to `Qwen/Qwen2.5-3B-Instruct`
- [ ] Update `scalpel.rs` agent instance metadata
- [ ] Verify server launch with: `mistralrs-server --port 8111 --isq Q4K -a qwen2 -m Qwen/Qwen2.5-3B-Instruct`
- [ ] Run inference smoke test
