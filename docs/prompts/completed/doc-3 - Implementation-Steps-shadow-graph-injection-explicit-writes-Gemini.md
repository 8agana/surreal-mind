---
id: doc-3
title: Implementation Steps - shadow graph injection + explicit writes (Gemini)
type: other
created_date: '2026-01-04 04:07'
updated_date: '2026-01-04 04:07'
---
# Implementation Steps - shadow graph injection + explicit writes (Gemini)

Linked task: `backlog/tasks/task-12 - Add-shadow-graph-injection-explicit-writes-for-Gemini.md`

## Goal
Add a shared “shadow graph” memory layer that Gemini can read/write. Default injection is ON and based on the prompt embedding. Gemini can write explicit shadow updates (structured blocks) that are always persisted (or fail the call if malformed).

## Defaults (chosen)
- injection default: **on**
- `shadow_top_k`: **5**
- `shadow_min_sim`: **0.75**
- `shadow_scope`: **global**
- explicit writes: **strict** (malformed write blocks cause tool failure)

## Data Model
Choose one of the two patterns below (recommended: unified table to minimize surface area). Either works.

### Option A (recommended): unified shadow table
`shadow_memory` with `kind` field and optional edges.

Fields (SCHEMAFULL or SCHEMALESS + indexes):
- `id` (SurrealDB record id)
- `kind`: "entity" | "observation" | "edge"
- `name`: string? (for entities)
- `text`: string (for observations)
- `source`: string? (edge)
- `target`: string? (edge)
- `rel_type`: string? (edge)
- `embedding`: array<float>
- `agent_source`: string ("gemini")
- `agent_instance`: string ("gemini")
- `scope`: "global" | "project" | "session"
- `tags`: array<string>?
- `confidence`: float?
- `created_at`: datetime (default now)
- `updated_at`: datetime?

Indexes:
- `scope`, `agent_source`, `kind`, `created_at`
- vector index on `embedding`

### Option B: separate tables
`shadow_entity`, `shadow_observation`, `shadow_edge` with similar fields + vector index on each.

## Tooling
### 1) Schema updates
- Update `src/server/schema.rs` to add shadow graph tables/indexes.
- Add vector index for embedding field (same dims as primary embedder).

### 2) Shadow search/write helpers
Add utilities in `src/utils` or a new `src/shadow` module:
- `shadow_add_*` to insert entities/observations/edges
- `shadow_search` to query top‑k by embedding with `min_sim` + `scope`

Use existing embedder for both writes and query embeddings.

### 3) Tool interfaces (optional but recommended)
Expose these tools so humans can inspect/manage:
- `shadow_memory_add` (write)
- `shadow_search` (read)
- optional `shadow_delete` / `shadow_decay`

### 4) delegate_gemini injection path
In `src/tools/delegate_gemini.rs` / `execute_gemini_call`:
- Add params:
  - `shadow_inject: bool` (default true)
  - `shadow_top_k: Option<usize>` (default 5)
  - `shadow_min_sim: Option<f32>` (default 0.75)
  - `shadow_scope: Option<String>` (default "global")
- Embed the prompt and query shadow graph for top‑k.
- Build a compact injected block:

```
Shadow Context (top 5, min_sim 0.75, scope=global):
- [obs:shadow_memory:abc123 | conf 0.72 | tags: mcp,tools]
  "delegate_gemini runs async-only; worker only spawns in HTTP transport."
```

- Prepend this block to the prompt before calling Gemini.
- Keep injection **small** (max ~1–2KB total). If longer, trim to top‑k or truncate lines.

### 5) Explicit shadow writes from Gemini output
- Define a strict write block format:

```
<shadow_write>
{"observations":[{"text":"...","confidence":0.8,"tags":["delegate","jobs"]}]}
</shadow_write>
```

- After Gemini returns, parse stdout response to extract this block.
- If block exists and JSON is malformed: **return tool error** (strict).
- On success, embed each observation text and insert into shadow graph with `agent_source="gemini"` and `scope` default.

### 6) PersistedAgent changes (if needed)
Since we are not relying on agent_exchanges, ensure injection logic does NOT depend on `PersistedAgent` context. Either:
- Add shadow injection at the tool handler level before calling the agent, or
- Update `PersistedAgent` to accept an optional injected prefix.

### 7) Docs + help
- Update `src/schemas.rs` with new delegate_gemini params.
- Update `src/tools/detailed_help.rs` with shadow param descriptions.
- Add short usage example in README/AGENTS.md (if desired).

### 8) Tests
- Unit test: extract and parse `<shadow_write>` block.
- Unit test: injection formatting and trimming logic.
- Optional: insert/search round‑trip with mock embedder (or use existing embedding test utilities).

## Decisions Needed (None unless you want changes)
- Defaults are already set above (on / top_k=5 / min_sim=0.75 / scope=global).
- We will use strict parsing for `<shadow_write>` blocks.

## Notes
- Keep shadow graph shared (no per‑agent partition beyond `agent_source` field).
- Only Gemini uses shadow injection for now, but other agents can use the same tools later.
- Do NOT reintroduce full conversation dumps; shadow graph should remain short, actionable memory.
