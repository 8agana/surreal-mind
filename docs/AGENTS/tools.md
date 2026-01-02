# Tools

- `legacymind_think` — unified thinking; continuity (`session_id`, `chain_id`, `previous_thought_id`, `revises_thought`, `branch_from`); `hint` (`debug|build|plan|stuck|question|conclude`); `injection_scale` 0–3; `tags`, `significance`; optional verification (`needs_verification`, `verify_top_k`, `min_similarity`, `evidence_limit`, `contradiction_patterns`).
- `legacymind_search` — KG + optional thoughts search; knobs: `target` (`entity|relationship|observation|mixed`), `include_thoughts`, `thoughts_content`, `top_k_memories`, `top_k_thoughts`, `sim_thresh`, `confidence_[g|l]te`, `date_from/to`, `order`, continuity links.
- `delegate_gemini` — Delegate prompts to Gemini CLI with persisted exchange tracking. `prompt` required, optional `task_name`, `model`.
- `agent_job_status` — Check status of a background agent job. `job_id` required.
- `list_agent_jobs` — List active/recent agent jobs.
- `cancel_agent_job` — Cancel a running agent job. `job_id` required.

- `curiosity_add` — add a curiosity entry (note) with optional tags/agent/topic/in_reply_to.
- `curiosity_get` — fetch recent curiosity entries (limit/since).
- `curiosity_search` — embedding search over curiosity entries with optional `recency_days`.
- `memories_create` — create KG `entity|relationship|observation`; supports `upsert`, `source_thought_id`, `confidence`, `data`.
- `memories_moderate` — review/decide staged KG candidates; `action` (`review|decide|review_and_decide`), filters + decisions payload.
- `maintenance_ops` — `health_check_embeddings`, `health_check_indexes`, `list_removal_candidates`, `export_removals`, `finalize_removal`, `reembed`, `reembed_kg`, `ensure_continuity_fields`, `echo_config`.
- `detailed_help` — deterministic schemas/prompts for tool discovery.
