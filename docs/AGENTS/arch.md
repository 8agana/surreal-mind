# Architecture & Guardrails

- KG-only injection; thoughts are not injected as context. Maintain embedding provider/dimension hygiene; re-embed on provider/dim change.
- Embeddings: OpenAI `text-embedding-3-small` (1536). Dev-only fallback: Candle `bge-small-en-v1.5` (384). Env knobs: `SURR_EMBED_PROVIDER`, `SURR_EMBED_MODEL`, `SURR_EMBED_STRICT`, `SURR_SKIP_DIM_CHECK`.
- Gemini CLI: `gemini-3-flash-preview` (default model), env knobs: `GEMINI_MODEL`, `GEMINI_TIMEOUT_MS`, `GEMINI_ENABLED`.
- Agent Jobs: Background task management (`agent_job_status`, `list_agent_jobs`, `cancel_agent_job`) for long-running operations.
- Delegate Gemini: Persisted exchange tracking via `delegate_gemini`.
- Retrieval knobs: `SURR_INJECT_T1/T2/T3` (0.6/0.4/0.25), `SURR_INJECT_FLOOR` (0.15), `SURR_KG_CANDIDATES`, `SURR_RETRIEVE_CANDIDATES`, `SURR_CACHE_MAX/WARM`, `SURR_KG_MAX_NEIGHBORS`, `SURR_KG_GRAPH_BOOST`, `SURR_KG_TIMEOUT_MS`.

- Verification: `SURR_VERIFY_TOPK`, `SURR_VERIFY_MIN_SIM`, `SURR_VERIFY_EVIDENCE_LIMIT`, `SURR_PERSIST_VERIFICATION`.
- Brain datastore (optional): `SURR_ENABLE_BRAIN`, `SURR_BRAIN_URL/NS/DB/USER/PASS`.
- Logging/runtime: `RUST_LOG` (default `surreal_mind=info,rmcp=info`), `MCP_NO_LOG` to keep stdio clean.
