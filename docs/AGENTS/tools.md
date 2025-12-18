# Tools

- `legacymind_think` — unified thinking; continuity (`session_id`, `chain_id`, `previous_thought_id`, `revises_thought`, `branch_from`); `hint` (`debug|build|plan|stuck|question|conclude`); `injection_scale` 0–3; `tags`, `significance`; optional verification (`needs_verification`, `verify_top_k`, `min_similarity`, `evidence_limit`, `contradiction_patterns`).
- `legacymind_search` — KG + optional thoughts search; knobs: `target` (`entity|relationship|observation|mixed`), `include_thoughts`, `thoughts_content`, `top_k_memories`, `top_k_thoughts`, `sim_thresh`, `confidence_[g|l]te`, `date_from/to`, `order`, continuity links.
- `inner_voice` — retrieval + synthesis (Grok primary → local fallback). Params: `query` (required), `top_k`, `floor`, `mix`, `include_private`, `include_tags/exclude_tags`, `auto_extract_to_kg`, `previous_thought_id`, `include_feedback`, `feedback_max_lines`. **Recency:** `recency_days` (date window filter), `prefer_recent` (ordering bias); exponential decay with half‑life (default 14d). Auto‑extract requires appended JSON `candidates`; no heuristic extraction.
- `memories_create` — create KG `entity|relationship|observation`; supports `upsert`, `source_thought_id`, `confidence`, `data`.
- `memories_moderate` — review/decide staged KG candidates; `action` (`review|decide|review_and_decide`), filters + decisions payload.
- `maintenance_ops` — `health_check_embeddings`, `health_check_indexes`, `list_removal_candidates`, `export_removals`, `finalize_removal`, `reembed`, `reembed_kg`, `ensure_continuity_fields`, `echo_config`.
- `detailed_help` — deterministic schemas/prompts for tool discovery.
