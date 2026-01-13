### Added
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
- **Scalpel Configuration**: Removed hardcoded default model from `src/clients/local.rs`. The `SURR_SCALPEL_MODEL` environment variable is now **mandatory**. This prevents silent failures/mismatches by forcing explicit configuration in `.env`.
- **Documentation**: Added Scalpel configuration section to `.env.example`.