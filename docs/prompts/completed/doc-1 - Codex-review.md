---
id: doc-1
title: Codex review
type: other
created_date: '2025-12-31 22:31'
---
# Codex review — kg_populate orchestrator

Related task: task-1 (Implement kg_populate orchestrator binary)

## Key additions / clarifications needed in task
- Field name mismatch: use `extracted_to_kg` (schema field), not `kg_extracted`; also set `extraction_batch_id` and `extracted_at` on thoughts.
- JSON schema for extraction prompt (from `docs/prompts/Archive/20251221-memories_populate-implementation.md`):
  - `entities`: `{ name, type, description, confidence }` (type: person|project|concept|tool|system)
  - `relationships`: `{ from, to, relation, description, confidence }`
  - `observations`: `{ content, context, tags, confidence }`
  - `boundaries`: `{ rejected, reason, context, confidence }`
  - Top-level `summary`
- KG tables + idempotent keys (per `src/tools/knowledge_graph.rs`):
  - `kg_entities` uniqueness by `name`
  - `kg_edges` uniqueness by `(source, target, rel_type)`
  - `kg_observations` uniqueness by `(name, data.source_thought_id)`
- `delegate_gemini` still exists and returns `{ response, session_id, exchange_id }`. It supports `model`, `cwd`, `timeout_ms`, and `fire_and_forget`.
- Prompt file should be created at `src/prompts/kg_extraction_v1.md` (task doc already specifies).
- Boundaries storage: no `boundary` table exists; decide to map boundaries into `kg_observations` with a type/tag if needed.
- Explicitly strip ```json fences before parsing.
- Define batch size defaults and any env overrides if desired.
- Logging: require per-batch counts and summary totals to satisfy Acceptance #8.
