# SurrealDB / SurrealMind — Upgrade Suggestions

A dated, living log of upgrade opportunities for SurrealMind and its dependency stack. Each entry: **date · what · status · rationale**. Entries are point-in-time observations — **verify current versions before acting**, a suggestion written weeks ago may already be done or obsolete.

Status tags: `DONE` · `INVESTIGATE` · `PENDING` · `DEFERRED` · `REJECTED`

---

## 2026-05-30

- **DiskANN → DEFERRED (premature). Current vector-search strategy VERIFIED = brute-force exact cosine, full scan.** This resolves the "verify the current strategy first" caveat on the 2026-05-28 DiskANN entry below. The search path (`src/tools/unified_search.rs`, `src/tools/wander.rs`, `src/server/db.rs`) builds `SELECT ..., vector::similarity::cosine(embedding, $q) AS similarity FROM <table> WHERE <filters> ORDER BY similarity DESC LIMIT $k` — it computes cosine for every matching row and sorts. The KNN operator `<|K|>` (the ONLY construct that exercises an HNSW index in SurrealDB) appears NOWHERE in the search code (grep count 0 across all three files). One HNSW index IS defined — `thoughts_embedding_idx ON thoughts FIELDS embedding HNSW` (`src/server/schema.rs:60`) — but it is **dead weight under the current query pattern**: maintained on every insert, never read. `kg_entities` has no embedding index at all → pure brute-force.

  **Why DiskANN is premature:** we are not index-bound — we are not even using the HNSW index we already have. A fancier ANN algorithm changes nothing while queries full-scan by function.

  **Cheaper real next steps, in order:**
  1. **HNSW wire-or-drop decision.** Either rewrite search to use `WHERE embedding <|$k|> $q` (actually exercise the existing index) OR drop `thoughts_embedding_idx` to stop paying insert cost for an unread index.
  2. **Scale threshold, not algorithm.** At ~2,400 thoughts, brute-force exact cosine is likely fine and may be *intentional* (exact > approximate; avoids HNSW + pre-filter composition pitfalls). Measure when full-scan latency actually hurts before adding ANY ANN index.
  3. **Confirm intent before changing.** SurrealDB HNSW composes poorly with WHERE pre-filtering (known limitation) — that may be the original reason brute-force was chosen. One-line confirm with whoever wrote the search path (Codex?) before touching it.

  *— finding by CC, autonomous exploration fire 2026-05-30 ~02:50 CDT. Read-only code inspection; nothing changed.*

---

## 2026-05-28

- **DONE — SurrealDB 3.0.4 → 3.1.2** (server + surreal-mind client). Coordinated cutover: client pre-built against crate 3.1.2 (clean compile, zero source-level API breaks), then stopped-clean server swap. Pre-existing surrealkv data (3.0.4-written) read cleanly under 3.1.2 — no on-disk format migration, confirmed by smoke test pulling historical thoughts. Backups (998M stopped fs copy + 737M/480K logical exports) at `~/Backups/surrealdb-cutover-20260528-3.1`; safe to clear after a few days.

- **INVESTIGATE — DiskANN approximate-nearest-neighbor index (new in SurrealDB 3.1).** SurrealMind stores 1536-dim embeddings (text-embedding-3-small) and does similarity retrieval on the `thoughts` table + KG entities on every `think`/`search`. **Verify the current similarity-search strategy first** (full-scan/brute-force vs. an existing HNSW/MTREE index). If retrieval is currently a full scan, a DiskANN index could materially cut latency as the thoughts table grows (already 2,393+ thoughts as of the 2.x→3.x migration). Worth a benchmark: current retrieval time vs. DiskANN-indexed, at current table size and projected. Do not assume benefit without measuring — small tables may not justify the index. **[UPDATE 2026-05-30: strategy verified — see top entry. Brute-force confirmed; HNSW defined-but-unused; DiskANN deferred as premature.]**

- **INVESTIGATE — rmcp 0.16.0 → 1.7.0.** Verified 2026-05-28 with `cargo search rmcp --limit 5`: crates.io latest is `rmcp = "1.7.0"` while SurrealMind remains pinned to `rmcp = { version = "0.16.0", features = ["macros", "transport-io", "transport-streamable-http-server", "transport-worker"] }` and `rmcp-macros 0.16.0` in `Cargo.lock`. This is not a casual patch bump; it crosses the pre-1.0 → 1.x boundary for the MCP serving layer. Recommendation: schedule as its own upgrade cycle after the SurrealDB 3.1 cutover settles. Acceptance bar: `cargo build --release`, full tests, local `/health`, MCP `tools/list`, at least one write tool (`think`) and one read tool (`search`) through Claude/Codex proxy, plus log review for transport/session behavior changes.

---

*Maintenance: add a dated entry whenever a dependency upgrade is considered, done, or deferred. This doc is the institutional memory for "why are we on version X and what's next." Keep newest date on top.*
