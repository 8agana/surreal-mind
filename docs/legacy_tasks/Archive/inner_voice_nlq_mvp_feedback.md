# Review: Inner Voice NLQ — MVP Implementation Plan
Date: 2025-09-02
Reviewer: Junie (JetBrains autonomous programmer)
Proposal under review: fixes/proposal-inner-voice-nlq-mvp.md

## Summary
The MVP plan is pragmatic and aligned with the repo’s development guidelines: ship a simple NLQ with deterministic parsing, filter-first then re-rank by cosine similarity, and provide a debuggable path with minimal infra. The proposal is coherent and can be delivered in ≤1 day if we keep scope tight.

## What’s Strong
- Scope control: “Make it work → Make it right” with Direct mode unchanged and a toggleable mode = Natural. Low blast radius.
- Deterministic, local-first approach: regex + chrono parsing, keyword lists, and Candle/OpenAI reuse for embeddings.
- Filter-then-rerank pipeline: reduces cost and improves precision on small candidate sets.
- Debuggability: `debug: true` returns parsed plan; `save` flag keeps persistence opt-in.
- Clear acceptance example and future phases.

## Gaps and Risks (with mitigation)
1) Temporal parsing specifics
- Risk: chrono-english crate/platform alignment and timezone handling can be flaky; parsing user strings like “two weeks ago” needs locale/timezone context.
- Mitigation: constrain supported phrases in Phase 1 (yesterday, last week, two weeks ago, since <date>); clamp to Config.runtime timezone. Provide a fallback to ignore time filter when parse fails, and include debug diagnostics.

2) Entity extraction ambiguity
- Risk: capitalized tokens are noisy (sentence starts, acronyms). Hard alias table can drift from KG content.
- Mitigation: use a small allowlist/aliases from config (toml) with case-insensitive matching; limit to 3 entities; treat as soft boost signals, not hard filters in Phase 1.

3) SQL injection/escaping
- Risk: building SurrealQL strings with user-provided keywords requires escaping quotes/percents.
- Mitigation: implement a minimal SQL-escape helper for ILIKE and literals; or use parameterization if available (SurrealQL prepared vars). For MVP, at least escape `'` → `\'` and `%/_` as needed.

4) Embedding dim mismatch and missing embeddings
- Risk: candidates may have missing or mismatched dims; proposal filters by equality but should count/skip efficiently.
- Mitigation: pre-filter records by `array::len(embedding) = expected_dim` in SQL WHERE to reduce client-side checks.

5) Candidate pool size
- Risk: small LIMIT risks recall issues; too large increases CPU for cosine.
- Mitigation: default 50–200 candidates, then top-k = limit. Expose SURR_TOP_K and SURR_SIM_THRESH in Config; for MVP, hard-code sane defaults (e.g., 100 candidates, take top 5–10).

6) Performance and timeouts
- Risk: embedding calls can stall if provider is remote.
- Mitigation: reuse existing embedder with retries/timeouts; log timing at debug for NLQ flow. Return an informative error on timeout and include `mode: natural` in response.

7) Observability
- Risk: hard to trace parsing decisions without logs.
- Mitigation: add a tracing span `inner_voice_nlq` with fields: limit, order, has_temporal, n_keywords, n_entities, candidate_count, reranked_count. Respect MCP_NO_LOG in tests.

8) Behavior drift vs. Direct mode
- Risk: confusion if Natural mode auto-saves or produces different content.
- Mitigation: keep `save: true` default but document it; set `mode` in response and include `sources` to justify output; in Direct mode behavior remains exactly as-is.

9) Testing
- Risk: untested edge cases (no matches, only temporal filter, entity-only query, multiple signals).
- Mitigation: add unit tests (no DB) for parsing functions; integration tests gated without DB should exercise the filter builder; DB-backed tests (gated by RUN_DB_TESTS) can exercise end-to-end with a tiny fixture.

## Specific Recommendations (actionable)
1) Data structures
- Keep QueryMode and InnerVoiceParams as proposed; default mode = Direct. Add serde(rename_all = "lowercase") on QueryMode.
- QueryFilter::to_surreal_where():
  - Ensure escaping for values: content ILIKE '%{escaped}%' using helper escape_like().
  - If providing entity soft-boost later, keep it out of WHERE; use ORDER BY CASE statements or post-rerank.

2) SQL generation
- Use SELECT meta::id(id) as id, content, embedding, created_at, significance FROM thoughts WHERE array::len(embedding) = {expected_dim} ...
- Order: respect parsed order, else default to created_at DESC when "recent" detected; otherwise none.
- Limit: clamp to [1, 50] with default from config (10).

3) Parsing helpers
- parse_quantifier(): also accept “latest”, “top”, “most recent”, numbers up to two digits. When conflicting signals, choose smallest parsed limit unless overridden by explicit params.limit.
- parse_temporal(): use chrono with timezone from Config (default America/Chicago). Supported phrases for MVP: yesterday, last week, two weeks ago, since <Month Day>.
- extract_sentiment_keywords(): normalize to lowercase; de-duplicate tokens.

4) Cosine similarity
- Reuse existing cosine implementation from server; ensure vectors are normalized (or normalize on the fly) for comparability.
- If no candidates after WHERE, short-circuit with message: "No relevant thoughts found matching your query." and return mode + parsed when debug.

5) Escaping utility (MVP-safe)
```rust
fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\'' => out.push_str("\\'"), // escape single quotes
            '%' => out.push_str("\\%"),
            '_' => out.push_str("\\_"),
            _ => out.push(ch),
        }
    }
    out
}
```
- Then generate: format!("content ILIKE '%{}%' ESCAPE '\\'", escape_like(k))

6) Config linkage
- For MVP, read timezone and default_limit from Config if already present; otherwise use constants with a TODO to unify later (matches repo plan).

7) Response shape
- Include: answer, sources (ids), mode, and optionally parsed when debug.
- Consider adding scores in debug to help tune thresholds.

## Test Plan (Phase 1)
- Unit tests (pure):
  - Quantifier parsing for: one/two/three, few/several/many, most recent/latest.
  - Temporal parsing: yesterday, last week, two weeks ago, since Aug 30; bad input → None.
  - Keyword extraction: excitement, frustration, accomplishment; mixed-case; duplicates removed.
  - SQL where builder escapes: input with quotes and percent.
- Integration tests (no DB):
  - Build-only checks: ensure inner_voice Natural mode compiles and returns structured response for empty candidate list (simulated).
- DB-backed (gated RUN_DB_TESTS=1):
  - Seed 3–5 thoughts spanning dates and sentiments; query “three accomplishments Sam got excited about two weeks ago” and assert ≤3 results, ordered by cosine.
  - Edge: query with no matches → graceful message.

## Security and Privacy
- Never log user content at info; only at debug with truncation (first 120 chars) and redaction if necessary.
- Sanitize SQL fragments (escaping) to avoid malformed queries. While SurrealQL is not classic SQL injection susceptible via our path, malformed strings can still break statements.

## Observability
- Add tracing span: `inner_voice_nlq` with fields listed above; guard with MCP_NO_LOG.
- Emit a single info log summarizing: candidates N, reranked M, limit, has_temporal, has_keywords.

## Next Steps (prioritized checklist)
1) Add parsing helpers (quantifier, temporal, sentiment, entity aliases) with unit tests.
2) Implement QueryMode::Natural path in inner_voice handler, behind params.mode.
3) Add SQL escaping helper and integrate into where builder.
4) Wire embeddings and cosine re-rank using existing embedder and dimensions; pre-filter by dim.
5) Add debug fields in response and a minimal info log.
6) Add a gated DB test with a tiny fixture, plus pure unit tests for parsers.
7) Document mode usage in README/tools help.

## Acceptance Re-stated (MVP)
- For the example query, the system produces the expected filter interpretation, generates a valid SQL with escaped predicates, returns up to 3 sources after cosine re-ranking, and synthesizes an answer from snippets or a graceful no-results message.

Overall: Proceed with MVP as proposed, with the above mitigations (escaping, timezone handling, dim pre-filtering, and tests). This keeps delivery under a day while reducing operational risks.
