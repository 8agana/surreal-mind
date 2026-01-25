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

## Maintenance & Help

| Tool | Description |
|------|-------------|
| `maintain` | System maintenance subcommands: `health_check_embeddings`, `health_check_indexes`, `reembed`, `reembed_kg`, `embed_pending`, `list_removal_candidates`, `export_removals`, `finalize_removal`, `ensure_continuity_fields`, `echo_config`, `rethink`, `populate`, `embed`, `wander`, `health`, `report`, `tasks`. |
| `howto` | Deterministic help for any tool. Params: `tool` (optional), `format` (`compact\|full`). Without `tool`, returns full roster. |

## Agent Delegation

All delegation tools include federation context and support `mode: "observe"` for read-only analysis.

| Tool | Description |
|------|-------------|
| `call_gem` | Delegate to Gemini CLI. Required: `prompt`, `cwd`. Optional: `model`, `resume_session_id`, `continue_latest`, `timeout_ms`, `tool_timeout_ms`, `expose_stream`, `mode` (`execute`\|`observe`), `max_response_chars` (default 100KB). |
| `call_cc` | Delegate to Claude Code CLI. Required: `prompt`, `cwd`. Optional: `model`, `resume_session_id`, `continue_latest`, `timeout_ms`, `tool_timeout_ms`, `expose_stream`, `mode`, `max_response_chars`. |
| `call_warp` | Delegate to Warp CLI (multi-model). Required: `prompt`, `cwd`. Optional: `model` (claude-4-5-haiku/sonnet/opus, gpt-5-2-codex-low/medium/high/xhigh/max, auto/auto-efficient/auto-genius), `timeout_ms`, `max_response_chars`, `task_name`, `mode`. One-shot executor, no resume. |
| `call_vibe` | Delegate to Vibe CLI. Required: `prompt`, `cwd`. Optional: `agent` (profile name), `mode`, `continue_latest`, `timeout_ms`, `max_response_chars`. Supports session continuation. |
| `call_status` | Check status of a background agent job. Required: `job_id`. |
| `call_jobs` | List active/recent agent jobs. Optional: `limit`, `status_filter`, `tool_name`. |
| `call_cancel` | Cancel a running agent job. Required: `job_id`. |

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

// Delegate to Gemini
{"tool": "call_gem", "arguments": {"prompt": "Analyze this code...", "task_name": "code_review"}}
```
