# SurrealMind → single-mind, Photography → ops-only

Goal: keep one consciousness (SurrealMind) for all thinking; move photography-specific MCP tools to a lean photography-ops service/CLI. No more photography “brain” tools or secondary DB handles inside SurrealMind.

## Tracks and owners
- **Codex (do here in LegacyMind repo)**: core code changes, config/doc updates, migration of photo thoughts into main mind DB, test updates, client config drafts if needed.
- **Grok (parallel/fast tasks)**: bulk search-and-remove, large-file refactors, or regeneration of schema docs if needed.

## High-level checklist (Codex unless noted)
1) **Tool surface cleanup**
   - Remove `photography_think/search/voice/moderate/memories` from `list_tools` and router.
   - Delete photography-specific schemas from `src/schemas.rs` if unused after removal.
2) **DB/config simplification**
   - Drop photo/brain secondary DB handles from `src/server/db.rs` and Config (`photo_*`, `brain_*` runtime fields).
   - Strip related env vars from `.env.example`, `.env.test`, `README.md`, `AGENTS.md`.
3) **Migration: photography thoughts → main mind**
   - Export photo-namespace thoughts and KG (if any) to JSON/SurQL.
   - Re-embed to primary provider/model/dim (`text-embedding-3-small`, 1536) using `reembed` if needed.
   - Import into main `thoughts` table with `tags=["photography"]`; keep continuity fields if present.
   - Remove/ignore old photo namespace after verification.
4) **Photography ops server/CLI**
   - Keep photography DB ops (import, dedupe, status, reports) as a separate binary/server (`photography-mcp` or CLI only) that talks to `photography/ops` DB.
   - Ensure it exposes only business tools (no embeddings/KG/thinking).
5) **Tests/CI**
   - Remove/adjust tests expecting photo tools in SurrealMind (`tool_schemas`, router/list_tools assertions).
   - Add smoke test for photography ops server (list_tools minimal) if split into separate binary.
6) **Docs/CHANGELOG**
   - Document the split rationale: one mind; photography MCP = ops.
   - Update quickstart/config sections; add migration notes and token/endpoints guidance.
7) **Client configs (Sam)**
   - Update Claude/Codex/Gemini MCP client entries: SurrealMind for thinking; photography ops for DB actions.

## Detailed tasks (with suggested executor)

### 1) Tool surface cleanup (Codex)
- [ ] `src/server/router.rs`: remove photo tool entries from `list_tools` and `call_tool`.
- [ ] `src/schemas.rs`: drop photo schemas no longer referenced.
- [ ] `src/config.rs` if any gating flags for photo tools—delete.

### 2) DB/config simplification (Codex)
- [ ] `src/server/db.rs`: remove `db_photo`/`db_brain` handles and conditionals; keep single `db`.
- [ ] `src/config.rs`: remove `photo_*`, `brain_*` runtime fields and loaders.
- [ ] `.env.example`, `.env.test`, `README.md`, `AGENTS.md`: remove photo/brain env knobs; keep single DB block.

### 3) Migration of existing photography thoughts (Codex)
- [ ] Dump photo namespace thoughts/KG: `surreal export` or existing backups (check `backups/photography-*`).
- [ ] Re-embed to 1536-dim if necessary (`cargo run --bin reembed -- --source photo_namespace --target main` or scripted).
- [ ] Import into main `thoughts` with `tags=["photography"]`; ensure `embedding_provider/model/dim` set.
- [ ] Verify retrieval: `legacymind_search` with `tags` filter returns expected photo thoughts.
- [ ] Remove/ignore old photo namespace; note in docs.

### 4) Photography ops server/CLI (Codex + Grok optional)
- [ ] Decide form: keep existing CLI only, or add slim MCP binary `photography-mind` exposing ops tools.
- [ ] If new binary: create `src/bin/photography_mind.rs` with its own router/list_tools (no embeddings).
- [ ] Ensure uses only `photography/ops` DB; no KG/embeddings code linked.
- [ ] Smoke test: list_tools shows only ops tools; basic command works.
- [ ] (Grok-friendly) Generate/update schema docs for photo ops tools if added.

### 5) Tests/CI (Codex)
- [ ] Update `tests/tool_schemas.rs` and router tests to remove photo expectations.
- [ ] Add minimal test for new photo ops server (if built).
- [ ] Ensure `cargo clippy -D warnings` and `RUN_DB_TESTS=1 cargo test --workspace` stay green.

### 6) Docs & changelog (Codex)
- [ ] `CHANGELOG.md`: add entry for single-mind split.
- [ ] `README.md`/`AGENTS.md`: clarify one-mind model; photography ops lives separately.
- [ ] If new binary, add run instructions and env vars.

### 7) Client configs (optional, later)
- [ ] Update MCP client configs to point SurrealMind -> thinking tools; Photography ops -> business tools (when requested).
- [ ] Remove old photo tools from client allowlists (when requested).

## Order of operations (safe path)
1) Remove photo tools from SurrealMind router/schemas/config.  
2) Clean docs/envs and tests accordingly.  
3) (Optional) Stand up photography ops server/binary; keep CLI working.  
4) Migrate existing photography thoughts into main mind and verify search.  
5) Update CHANGELOG/docs; notify client configs.  

## Risks & mitigations
- **Client breakage**: clients calling removed photo tools → mitigate by updating configs and documenting early.
- **Data loss**: migration must be backup-first; keep photo namespace dumps until post-verify.
- **Dim mismatch**: ensure re-embed to active model/dim before import.

## Notes
- Tagging approach keeps all cognition in one KG while allowing scoped retrieval (`tags=["photography"]`).
- Photography ops service should remain read/write on `photography/ops` only; no cross-writes into mind DB.
