# Tools

- `legacymind_think` — unified thinking; continuity (`session_id`, `chain_id`, `previous_thought_id`, `revises_thought`, `branch_from`); `hint` (`debug|build|plan|stuck|question|conclude`); `injection_scale` 0–3; `tags`, `significance`; optional verification (`needs_verification`, `verify_top_k`, `min_similarity`, `evidence_limit`, `contradiction_patterns`).
- `legacymind_search` — KG + optional thoughts search; knobs: `target` (`entity|relationship|observation|mixed`), `include_thoughts`, `thoughts_content`, `top_k_memories`, `top_k_thoughts`, `sim_thresh`, `confidence_[g|l]te`, `date_from/to`, `order`, continuity links.
- `legacymind_search`: **Unified Search** for thoughts and memories. Supports filtered, semantic, and hybrid retrieval.
- `legacymind_wander`: **Interactive Exploration** tool. Supports random jumps, semantic steps, and meta-traversal to explore the graph seredipitously.
- `delegate_gemini` — Delegate prompts to Gemini CLI with persisted exchange tracking. `prompt` required, optional `task_name`, `model`.
- `agent_job_status` — Check status of a background agent job. `job_id` required.
- `list_agent_jobs` — List active/recent agent jobs.
- `cancel_agent_job` — Cancel a running agent job. `job_id` required.

- `memories_create` — create KG `entity|relationship|observation`; supports `upsert`, `source_thought_id`, `confidence`, `data`.
- `memories_moderate` — review/decide staged KG candidates; `action` (`review|decide|review_and_decide`), filters + decisions payload.
- `maintenance_ops`: **System Maintenance** tools (archival, removals, health checks).
- `detailed_help` — deterministic schemas/prompts for tool discovery.

```
