# Tools

## Core Cognitive Tools

| Tool | Description |
|------|-------------|
| `think` | Unified thinking with continuity links (`session_id`, `chain_id`, `previous_thought_id`, `revises_thought`, `branch_from`). Modes via `hint`: `debug\|build\|plan\|stuck\|question\|conclude`. Injection via `injection_scale` 0–3. Optional verification: `needs_verification`, `verify_top_k`, `min_similarity`, `evidence_limit`, `contradiction_patterns`. |
| `search` | Unified KG + thoughts retrieval. Params: `target` (`entity\|relationship\|observation\|mixed`), `include_thoughts`, `thoughts_content`, `top_k_memories`, `top_k_thoughts`, `sim_thresh`, `confidence_[g\|l]te`, `date_from/to`, `order`, continuity filters. Supports direct ID lookup via `query.id` and `forensic` mode for provenance. |
| `remember` | Create KG `entity\|relationship\|observation`. Supports `upsert`, `source_thought_id`, `confidence`, `data`. |
| `wander` | Explore the knowledge graph serendipitously. Modes: `random`, `semantic`, `meta`, `marks`. Returns actionable guidance for improving KG quality. |
| `rethink` | Revise or mark knowledge graph items for correction. Modes: `mark` (flag for review), `correct` (apply fix with provenance). |
| `corrections`| List recent `correction_events` to inspect the learning journey of the KG. |
| `journal` | Research thread management over the KG: create threads, add entries, review dashboard state, and update thread status. |

## Maintenance & Help

| Tool | Description |
|------|-------------|
| `maintain` | System maintenance subcommands: `health_check_embeddings`, `health_check_indexes`, `reembed`, `reembed_kg`, `embed_pending`, `list_removal_candidates`, `export_removals`, `finalize_removal`, `ensure_continuity_fields`, `echo_config`, `rethink`, `populate`, `embed`, `wander`, `health`, `report`, `tasks`. |
| `howto` | Deterministic help for any tool. Params: `tool` (optional), `format` (`compact\|full`). Without `tool`, returns full roster. |
| `test_notification` | Diagnostic tool that sends a test logging notification to the client. Required: `message`. Optional: `level` (`debug\|info\|notice\|warning\|error\|critical\|alert\|emergency`, default `info`). |

## Usage Examples

```json
// Think with debug hint
{"tool": "think", "arguments": {"content": "Investigating the null pointer exception...", "hint": "debug"}}

// Search entities by name
{"tool": "search", "arguments": {"query": {"name": "SurrealDB"}, "target": "entity"}}

// Create an entity
{"tool": "remember", "arguments": {"kind": "entity", "data": {"name": "Rust", "entity_type": "language"}}}

// Explore the graph
{"tool": "wander", "arguments": {"mode": "semantic", "current_thought_id": "thoughts:abc123"}}

```
