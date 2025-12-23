# Architecture & Guardrails

- KG-only injection; thoughts are not injected as context. Maintain embedding provider/dimension hygiene; re-embed on provider/dim change.
- Embeddings: OpenAI `text-embedding-3-small` (1536). Dev-only fallback: Candle `bge-small-en-v1.5` (384). Env knobs: `SURR_EMBED_PROVIDER`, `SURR_EMBED_MODEL`, `SURR_EMBED_STRICT`, `SURR_SKIP_DIM_CHECK`.
- Gemini CLI: `gemini-2.5-pro` (default model), env knobs: `GEMINI_MODEL`, `GEMINI_TIMEOUT_MS`, `GEMINI_ENABLED`.
- Retrieval knobs: `SURR_INJECT_T1/T2/T3` (0.6/0.4/0.25), `SURR_INJECT_FLOOR` (0.15), `SURR_KG_CANDIDATES`, `SURR_RETRIEVE_CANDIDATES`, `SURR_CACHE_MAX/WARM`, `SURR_KG_MAX_NEIGHBORS`, `SURR_KG_GRAPH_BOOST`, `SURR_KG_TIMEOUT_MS`.
- Inner Voice: defaults `mix=0.6`, `topk_default=10`, `min_floor=0.15`, `max_candidates_per_source=150`, planner off by default (`SURR_IV_PLAN`). Recency: half-life default 14d (`SURR_IV_RECENCY_HALF_LIFE_DAYS`); optional window (`SURR_IV_RECENCY_DEFAULT_DAYS`, e.g., 30) and bias flag (`SURR_IV_PREFER_RECENT`).
- Verification: `SURR_VERIFY_TOPK`, `SURR_VERIFY_MIN_SIM`, `SURR_VERIFY_EVIDENCE_LIMIT`, `SURR_PERSIST_VERIFICATION`.
- Brain datastore (optional): `SURR_ENABLE_BRAIN`, `SURR_BRAIN_URL/NS/DB/USER/PASS`.
- Logging/runtime: `RUST_LOG` (default `surreal_mind=info,rmcp=info`), `MCP_NO_LOG` to keep stdio clean.
