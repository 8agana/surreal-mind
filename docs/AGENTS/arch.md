# Architecture & Guardrails

- KG-only injection; thoughts are not injected as context. Maintain embedding provider/dimension hygiene; re-embed on provider/dim change.
- Embeddings: OpenAI `text-embedding-3-small` (1536) is the supported deployed runtime path. No mixed-dimension fallback in normal operation. Model and dimensions come from `surreal_mind.toml` `[system]`; `SURR_EMBED_PROVIDER` can override the configured provider. Other environment knobs include `SURR_EMBED_STRICT`, `SURR_SKIP_DIM_CHECK`, `SURR_EMBED_RETRIES`, and `SURR_EMBED_RPS`.
- Google CLI delegation: Runtime provider is selected with `SM_AGENT_PROVIDER`, `GOOGLE_CLI_PROVIDER`, or `SURR_GOOGLE_CLI_PROVIDER` (`antigravity` default, `gemini` rollback). Gemini uses `GEMINI_MODEL`/`GEMINI_TIMEOUT_MS`; Antigravity uses `ANTIGRAVITY_MODEL`/`AGY_MODEL`, `ANTIGRAVITY_TIMEOUT_MS`/`AGY_TIMEOUT_MS`.
- SurrealDB 3.x: `think`/injection retrieval uses DB-side cosine scoring during KG memory injection to avoid large WS embedding payload decode issues.
- Retrieval knobs: `SURR_INJECT_T1/T2/T3` (0.6/0.4/0.25), `SURR_INJECT_FLOOR` (0.15), `SURR_KG_CANDIDATES`, `SURR_RETRIEVE_CANDIDATES`, `SURR_CACHE_MAX/WARM`, `SURR_KG_MAX_NEIGHBORS`, `SURR_KG_GRAPH_BOOST`, `SURR_KG_TIMEOUT_MS`.

- Verification: `SURR_VERIFY_TOPK`, `SURR_VERIFY_MIN_SIM`, `SURR_VERIFY_EVIDENCE_LIMIT`, `SURR_PERSIST_VERIFICATION`.
- Logging/runtime: `RUST_LOG` (default `surreal_mind=info,rmcp=info`), `MCP_NO_LOG` to keep stdio clean.

## Environment provenance and guard boundaries

- Treat inherited environment variables and values loaded from `.env` as configuration input supplied through the operator environment, not authenticated identity or a permit. `dotenvy` merges file values into the process-global environment; after that merge, `std::env::var` cannot identify whether a value came from a shell, launchd, a local `.env`, or an ancestor `.env`.
- `SURR_ENV_FILE` pins shared dotenv-loading callers to one exact file. When it is unset, preserve each caller's established fallback: `Config::load` tries `./.env`, then `../.env` only when neither `SURR_DB_URL` nor `OPENAI_API_KEY` is present; the other shared callers use the ordinary upward-searching `dotenvy::dotenv()` fallback. Do not collapse those distinct unset behaviors.
- An environment-keyed guard is operational policy and accidental-action friction, not an authorization or security boundary. A deterministic wrapper such as `scripts/test_db.sh` can sanitize inherited variables and pin `SURR_ENV_FILE` to control its own subprocess, but read ordering cannot prove that a process-wide value was not dotenv-sourced.
- Future security or authorization boundaries must use an authenticated capability, OS/file permissions, or an explicit programmatic permit that dotenv cannot manufacture. Do not describe an env denylist, capture-before-load ordering, or an opt-in variable as a structural barrier.
- The deployed fake-embedder barrier is compile-time exclusion: `test-embedder` is a non-default feature, so a plain `cargo build --release` does not compile `FakeEmbedder` or the `"fake"` provider arm. A release built with `--features test-embedder` or `--all-features` does include that arm; `SURR_ALLOW_FAKE_EMBEDDER=1` is operational opt-in policy inside such a build, not a security boundary. If a deployed service is ever built with either feature set, documentation is no longer sufficient and any security permit must leave the environment.
