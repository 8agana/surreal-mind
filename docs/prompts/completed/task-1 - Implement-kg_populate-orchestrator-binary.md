---
id: task-1
title: Implement kg_populate orchestrator binary
status: Done
assignee: []
created_date: '2025-12-31 22:10'
updated_date: '2025-12-31 23:53'
labels:
  - kg-orchestration
  - surreal-mind
  - maintenance-binary
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rebuild the knowledge graph extraction orchestrator as a standalone Rust binary (src/bin/kg_populate.rs). This bridges raw Gemini compute with knowledge graph storage after memories_populate was removed.

Related Documentation:
- doc-1: Codex review (Technical Spec)
- doc-2: CC review (Feasibility)
- doc-3: Implementation Guide (Codebase Patterns)
- doc-4: Implementation Summary
- doc-5: Testing Plan
- doc-6: Test Execution Log

Core flow:
1. Load SurrealDB/Gemini config
2. Fetch unextracted thoughts (WHERE extracted_to_kg = false)
3. Batch them to Gemini via delegate_gemini with extraction prompt
4. Parse JSON output (entities, relationships, observations)
5. Upsert to knowledge graph tables
6. Mark thoughts as extracted

Related prompt: docs/prompts/20251231-codex-kg-populate.md
<!-- SECTION:DESCRIPTION:END -->

## Overview
Rebuild the knowledge graph extraction orchestrator as a standalone maintenance binary (`src/bin/kg_populate.rs`). This connects raw Gemini compute (`delegate_gemini`) with knowledge graph CRUD operations to enable idempotent batch processing of thought backlogs.

## Implementation Steps

### Step 1: Extract and Store Prompt
- Source: `docs/prompts/Archive/20251221-memories_populate-implementation.md`
- Destination: `src/prompts/kg_extraction_v1.md`
- Ensure prompt requests JSON output with schema for entities, relationships, observations

### Step 2: Implement `src/bin/kg_populate.rs`
Create binary with the following logic:
1. Load config (SurrealDB credentials, Gemini model)
2. Fetch unextracted thoughts: `WHERE extracted_to_kg = false OR extracted_to_kg IS NONE`
3. Load prompt from `src/prompts/kg_extraction_v1.md` and inject batch
4. Execute via Gemini (handle markdown code fence stripping)
5. Parse JSON and upsert to `kg_entities`, `kg_edges`, `kg_observations`
6. Update thoughts: `SET extracted_to_kg = true, extracted_at = time::now()`
7. Report stats; support loop mode

### Step 3: Refactoring (Optional)
- Extract shared `GeminiClient` or markdown stripping logic into `src/clients/gemini.rs` to avoid duplication

## Requirements
- **Idempotency**: No re-processing of marked thoughts
- **Safety**: Graceful handling of malformed JSON (log, skip, retry)
- **Observability**: Clear progress logging/status bars

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Binary compiles and runs: cargo run --bin kg_populate
- [x] #2 Fetches unextracted thoughts from SurrealDB
- [x] #3 Calls delegate_gemini with extraction prompt
- [x] #4 Parses JSON response (handles markdown code fences)
- [x] #5 Upserts entities/relationships/observations to KG tables
- [x] #6 Marks processed thoughts as kg_extracted = true
- [x] #7 Idempotent - safe to run multiple times
- [x] #8 Logs progress clearly
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## IMPLEMENTATION NOTES (from doc-3 investigation)

**Codebase patterns verified via Serena:**
- Config: Use Config::load() for TOML + env loading
- DB connection: Standard pattern from reembed.rs (Surreal::new → signin → use_ns/use_db)
- delegate_gemini: Use PersistedAgent wrapper, returns { response, session_id, exchange_id }
- KG upserts: Entities by name, Edges by (src,dst,rel), Observations by (name,source_thought_id)
- JSON parsing: Strip ```json fences before serde_json::from_str
- Error handling: anyhow::Result for binaries
- Logging: println! with emoji (🚀 ✅ 📊 🔄)

**Critical schema correction:**
- Field is `extracted_to_kg` (bool, default false), NOT `kg_extracted`
- Also set: `extraction_batch_id` (string), `extracted_at` (datetime)

**See doc-3 for complete patterns and code examples**
<!-- SECTION:NOTES:END -->
