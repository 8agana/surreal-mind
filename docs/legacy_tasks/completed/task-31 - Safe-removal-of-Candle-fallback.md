# Task 31: Remove Candle/Local Embeddings Fallback

**Status:** Completed
**Owner:** Gemini (Antigravity)
**Context:** We are removing the local Candle-based embedding fallback. It is better for the system to fail explicitly if OpenAI is unreachable than to silently generate 384-dim embeddings that corrupt the 1536-dim vector space.

## Architecture Change

- **Current State:** `surreal-mind` attempts to use OpenAI `text-embedding-3-small` (1536 dims). If that fails or isn't configured, it falls back to a local BERT model (384 dims).
- **Desired State:** `surreal-mind` ONLY uses OpenAI. If unreachable, tools return a clear error.

## Proposed Changes

### 1. Dependency Removal
- Remove `candle-core`, `candle-nn`, `candle-transformers`, `tokenizers`, `hf-hub` from `Cargo.toml`.
- This will significantly reduce binary size and compile time.

### 2. Code Removal
- Remove `src/clients/local_bert.rs`.
- Remove fallback logic in `src/clients/openai.rs` or `src/tools/knowledge_graph.rs`.

### 3. Error Handling
- Update embedding clients to return `AgentError::DependencyFailure` when the provider is unreachable, rather than attempting fallback.

## Acceptance Criteria

1.  **Clean Compile**: `cargo build --release` succeeds without Candle dependencies.
2.  **Explicit Failure**: Disabling internet/API key causes `reembed` or `remember` to fail with a clear "OpenAI unreachable" error.
3.  **No Dimension Mismatch**: The system can no longer accidentally generate 384-dim vectors.

## Federation Notes
- **Reliability**: Improves data hygiene by preventing mixed-dimension vectors.
- **Performance**: Smaller binary, faster build.
