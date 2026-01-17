## [Unreleased] - 2026-01-17

### Changed

- **call_codex Tool**: Refactored to synchronous execution - returns response directly in MCP call instead of async job queue. Removed worker polling pattern for simpler, more reliable operation.
- **CodexClient**: Added `--skip-git-repo-check` flag for execution in any directory. Fixed NDJSON parser to handle Codex's `item.aggregated_output` format and `thread_id` extraction.
- **Codex Model Configuration**: Default model and available models dropdown now read from environment variables (`CODEX_MODEL` and `CODEX_MODELS`) instead of hardcoded - no rebuild required to change model list.
- **call_gem Native Resume**: Added `resume_session_id` and `continue_latest` parameters. Gemini CLI auto-saves all sessions - use `continue_latest: true` for `--resume` (latest) or `resume_session_id` for specific session.

### Fixed

- **delegate_gemini Worker**: Fixed job stealing bug - worker now filters by `tool_name = 'delegate_gemini'` to prevent claiming jobs from other tools like call_codex.
- **CodexClient Session Resume**: Fixed CLI argument ordering per v0.79.0+ docs. Resume is a subcommand of exec with strict ordering: `codex exec resume <id> "prompt" [flags]`. Prompt now placed before flags.

### Removed

- **PersistedAgent Wrapper**: Removed fake memory/statefulness layer that concatenated previous exchanges into prompts. The `persisted.rs` module and related `agent_exchanges`/`tool_sessions` DB writes are removed.
- **call_codex Async Worker**: Removed background job queue pattern in favor of synchronous execution.
- **call_gem Async Worker**: Removed background job queue pattern in favor of synchronous execution. Tool now returns response directly.

---

### Added

- **call_codex Tool**: Added Codex CLI delegation with async job tracking, resume options, and stream metadata capture.
- **Graceful Embedding Degradation**: Thoughts are now saved before embedding, preventing data loss when the OpenAI embedding API is unavailable. Failed embeddings can be retried later via `maintain embed_pending`. Adds `embedding_status` field to thoughts table (values: `pending`, `complete`, `failed`).
- **Phase 1: Schema & Data Model**: Implemented the initial schema for the REMini & Correction System, adding Mark fields (`marked_for`, `mark_type`, `mark_note`, `marked_at`, `marked_by`) to thoughts, kg_entities, and kg_observations tables, and creating the CorrectionEvent table with fields for provenance tracking.
- **Phase 2: rethink Tool - Mark Mode**: Implemented the `rethink` MCP tool with mark creation capability.
- **Phase 3: wander --mode marks**: Added capability to surface and filter marks in the `wander` tool.
- **Phase 4: rethink Tool - Correct Mode**: Implemented full correction provenance with CorrectionEvent tracking and derivative cascading.
- **Phase 5: gem_rethink Process**: Created a specialized binary for autonomous background correction processing by Gemini.
- **Phase 6: REMini Wrapper**: Implemented a unified maintenance orchestrator (`remini` CLI) to manage background tasks.
- **Phase 7: Forensic Queries**: Added `--forensic` flag to the `search` tool to expose correction chains and provenance data.
- **Phase 8: Confidence Decay**: (Foundation) Added confidence fields and decay tracking logic to the core schemas.
- **Phase 9: Corrections Tool**: Integrated the standalone `corrections` tool and mapped it into the `maintain` surface.

### Removed

- **Scalpel Tool**: Fully removed the scalpel tool and local delegation infrastructure to free port 8111 and improve reliability. Scalpel was unreliable on the 32GB Studio; use remote `call_gem` for delegation instead.
- **Scalpel Environment Variables**: Removed all scalpel-related environment variables (`SURR_SCALPEL_MODEL`, `SURR_SCALPEL_ENDPOINT`, `SURR_SCALPEL_MAX_TOKENS`, `SURR_SCALPEL_TIMEOUT_MS`) from `.env` and `.env.example` files.

### Changed

- **Thought Persistence**: Avoid writing empty embeddings during initial thought creation so HNSW indexing doesn't reject the record; embedding is only set after a valid vector is produced.
- **Thought Schema**: Set `thoughts.embedding` to `option<array<float>>` with `DEFINE FIELD OVERWRITE` so the migration applies on startup; initial create uses `embedding: NONE` to pass schema validation before embedding is computed.
- **Thought Create Validation**: Thought creation now returns `meta::id` and checks the response to surface DB errors instead of failing silently.
- **Scalpel Configuration**: Removed hardcoded default model from `src/clients/local.rs`. The `SURR_SCALPEL_MODEL` environment variable is now **mandatory**. This prevents silent failures/mismatches by forcing explicit configuration in `.env`.
- **Documentation**: Added Scalpel configuration section to `.env.example`.
