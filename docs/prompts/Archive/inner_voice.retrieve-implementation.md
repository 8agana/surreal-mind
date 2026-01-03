# inner_voice.retrieve — Retrieval-Only Tool (Spec)

Date: 2025-09-04
Owner: Codex
Status: Approved spec (ready for implementation behind a feature gate)

## Purpose
- Provide a gated, retrieval‑only MCP tool that turns a natural‑language query into provenance‑rich snippets from both Thoughts and KG for external synthesis (e.g., Grok 256k session or Gemini REPL with saved chats).
- No in‑repo LLM calls and no natural‑language “answers”. The tool returns structured data only.

## Non‑Goals
- No synthesis inside the MCP server.
- No provider/session orchestration here (handled by external agent/session).

## Constraints & Guardrails
- Feature gate: tool is disabled unless `SURR_ENABLE_INNER_VOICE=1`.
- Dim safety: always filter by `embedding_dim == active.dim` before any cosine similarity.
- No provider fallback at call time; embedder/provider chosen at startup.
- No UNION when querying KG tables; use separate SELECTs and merge client‑side.
- Privacy: exclude `is_private=true` thoughts unless explicitly allowed; support include/exclude tag filters.
- Logging: honor `MCP_NO_LOG`. When set, suppress detailed logs; emit only minimal counters/latency if needed.
- Dedupe: per `(table,id)` and by stable content hash `blake3(normalized_text)` (normalize: lowercase + collapse whitespace).
- Text cap: 600–800 chars per snippet; cut on sentence boundary when possible.

## Tool Name
- Preferred: `inner_voice.retrieve` (clear lineage)
- Alternative: `snippets.retrieve` (if you prefer to avoid “inner_voice” until synthesis returns)

## Parameters
- `query` (string, required): Natural‑language input.
- `top_k` (int, default 10; clamp 1–50): Max snippets to return; hard‑cap prevents memory spikes.
- `floor` (float, default from config; may adapt down to `min_floor`): Minimum cosine similarity.
- `mix` (float, default 0.6): Ratio KG:Thoughts. Bounds: `0.0 ≤ mix ≤ 1.0`. `0.0 = thoughts‑only`, `1.0 = KG‑only`.
- `include_private` (bool, default false): Include thoughts where `is_private=true`.
- `include_tags` (string[], default []): Only include items containing at least one of these tags.
- `exclude_tags` (string[], default []): Exclude items containing any of these tags.

## Output Schema (exact)
```json
{
  "snippets": [
    {
      "id": "t:abc123",
      "table": "thoughts",
      "source_type": "thought",              // "kg_entity" | "kg_observation"
      "origin": "human",                     // "human" | "tool" | "model"
      "trust_tier": "green",                 // green|amber|red (see mapping)
      "created_at": "2025-09-02T21:11:00Z",
      "text": "…capped snippet…",
      "score": 0.71,
      "content_hash": "8f2c…",
      "span_start": 0,                         // optional
      "span_end": 142                          // optional
    }
  ],
  "diagnostics": {
    "provider": "openai",
    "model": "text-embedding-3-small",
    "dim": 1536,
    "k_req": 10,
    "k_ret": 9,
    "kg_candidates": 120,
    "thought_candidates": 95,
    "floor_used": 0.15,
    "latency_ms": 73
  }
}
```

### Trust tier mapping
- KG → `green`.
- Thoughts: `origin=human|logged` → `green`; `origin=tool` → `amber`; `origin=model` → `red`.

## Behavior
1) Gate: Return error if `SURR_ENABLE_INNER_VOICE != "1"`.
2) Embed: Create query embedding via active embedder; record `{provider, model, dim}`.
3) Candidates: Fetch up to `min(3×top_k, MAX_CANDIDATES_PER_SOURCE)` per source (Thoughts, KG entity/observation separately) with `embedding_dim == active.dim`. Default `MAX_CANDIDATES_PER_SOURCE=150` (configurable).
4) Rank: Compute cosine similarity per source; drop below `floor` (adapt down to `min_floor` if needed to reach at least 1 per source when both exist).
5) Allocate: `kg_slots = round(mix * top_k)`, `thought_slots = top_k - kg_slots`.
   - Guarantee ≥1 per source when both sources are non‑empty and `0 < mix < 1`.
   - If one source underflows after floor/dedupe, backfill from the other to hit `top_k`.
6) Dedupe: First by `(table,id)`, then by `content_hash` across sources.
7) Cap text: 600–800 chars; prefer sentence boundary; attach optional `span_start/span_end` when available.
8) Re‑rank: Global sort by `score` (desc) for final list.
9) Return: `snippets[]` and `diagnostics{…}`. Never include model‑synthesised content.

### Empty result handling
- If no candidates meet `floor` (even after adaptive down to `min_floor`), return `200 OK` with `snippets: []` and diagnostics including `no_results: true` and `reason: "floor_excluded_all"` or `"no_candidates"`.

### Embedder failure
- If the embedder is unavailable or errors, return a structured error (see Error Handling) and do not fallback providers at call time.

## Config
- `SURR_ENABLE_INNER_VOICE=1` (default off)
- `SURR_INNER_VOICE_MIX` (default 0.6)
- `SURR_INNER_VOICE_TOPK_DEFAULT` (default 10)
- `SURR_INNER_VOICE_MIN_FLOOR` (default 0.10–0.15)
- `SURR_INNER_VOICE_MAX_CANDIDATES_PER_SOURCE` (default 150)
- Reuse embedder settings: `SURR_EMBED_PROVIDER`, `SURR_EMBED_MODEL`, `SURR_EMBED_DIM`

### Config validation
- Enforce bounds at startup:
  - `0.0 ≤ SURR_INNER_VOICE_MIX ≤ 1.0` (out‑of‑range → clamp + warn).
  - `1 ≤ SURR_INNER_VOICE_TOPK_DEFAULT ≤ 50` (out‑of‑range → clamp + warn).
  - `0.0 < SURR_INNER_VOICE_MIN_FLOOR < 1.0` (invalid → fallback to 0.15 with warn).
  - `SURR_INNER_VOICE_MAX_CANDIDATES_PER_SOURCE ≥ top_k` (invalid → set to `max(3×top_k, 150)` with warn).

## Handler Sketch (pseudocode)
```rust
pub async fn inner_voice_retrieve(&self, p: Params) -> Result<RetrieveOut> {
    guard_feature_enabled()?;
    let cfg = self.config();
    // Embed with graceful error mapping
    let qvec = self.embedder.embed(&p.query).await.map_err(|e| Error::embedder_unavailable(e))?; // must be dim==cfg.dim

    // Build filters
    let thoughts_q = build_thoughts_query(&cfg, &p);
    let kg_q = build_kg_query(&cfg, &p);

    // Separate fetches
    let cap = (3*p.top_k).min(cfg.max_candidates_per_source);
    let t_candidates = fetch_ranked_thoughts(thoughts_q, qvec, cap).await?;
    let k_candidates = fetch_ranked_kg(kg_q, qvec, cap).await?;

    // Apply floor (adaptive down to min_floor if needed for coverage)
    let (t_hits, k_hits, floor_used) = apply_floor_with_adapt(t_candidates, k_candidates, p.floor, cfg.min_floor);

    // Allocate by mix with guarantees
    let (t_take, k_take) = allocate_slots(p.top_k, p.mix, &t_hits, &k_hits);

    // Dedupe and cap text
    let mut picked = pick_and_dedupe(t_hits, k_hits, t_take, k_take);
    for s in &mut picked { cap_text_sentence(s, 800); hash_content(s); tier_from_origin(s); }

    // Global re-rank and trim
    picked.sort_by(|a,b| b.score.partial_cmp(&a.score).unwrap());
    picked.truncate(p.top_k);

    Ok(RetrieveOut { snippets: picked, diagnostics: Diagnostics { /* provider/model/dim… plus per‑source counts and timings */ } })
}
```

## Example Request
```json
{
  "tool": "inner_voice.retrieve",
  "arguments": {
    "query": "Patterns in Federation coordination failures",
    "top_k": 10,
    "mix": 0.6,
    "include_private": false,
    "exclude_tags": ["private"]
  }
}
```

## External Synthesis (informative)
- Feed the returned `snippets[]` into a long‑context session (e.g., Grok 256k or Gemini REPL):
  - Hydrate once with a Memory Pack (KG + selected thoughts) and guardrails (grounded answers, refusal on no sources).
  - Append deltas per turn and cite snippet IDs in answers.
- This MCP tool never invokes an LLM.

## Tests
- Dim filter: No mixed‑dim comparisons; simulated mismatched rows are excluded.
- Mix allocation: `mix=0.6, top_k=10` → 6 KG / 4 Thoughts when available; fallback fills when one source underflows.
- Privacy/tags: `include_private=false` excludes; include/exclude tags respected.
- Dedupe: identical text across tables deduped via `content_hash`.
- Text cap: outputs are ≤800 chars and cut on sentence boundaries where possible.
- Empty results: returns `snippets: []` with `no_results=true` and reason when floor excludes all.
- Embedder failure: returns structured `embedder_unavailable` error; no provider fallback.

## Acceptance Criteria
- Tool gated by `SURR_ENABLE_INNER_VOICE=1` (off by default).
- Output schema matches exactly; contains no synthesized text.
- Dim safety, privacy, no‑UNION, dedupe, and diagnostics implemented as specified.

## Error Handling (structured)
- Return typed errors with stable codes; avoid leaking internals:
  - `feature_disabled`: gate is off.
  - `invalid_params`: e.g., empty query after trim, out‑of‑range bounds (post‑clamp still invalid).
  - `embedder_unavailable`: upstream embedder error/timeouts.
  - `db_error`: Surreal query failed.
  - `internal_error`: unexpected failure.
- For non‑error but empty results, prefer `200` with `diagnostics.no_results=true`.

## Logging Strategy
- When `MCP_NO_LOG=1`: log nothing beyond minimal success/error counters.
- Otherwise (info level): log only non‑content metrics: `{provider, model, dim, k_req, floor, floor_used, kg_candidates, thought_candidates, k_ret, latency_ms}`.
- Debug level (opt‑in): may include snippet ids and tables, never text.

## Content Normalization (for hashing)
- Steps applied before `blake3`:
  1) Unicode NFKC normalization.
  2) Lowercase.
  3) Replace all Unicode whitespace sequences with a single ASCII space.
  4) Trim leading/trailing spaces.
  5) Remove zero‑width/control chars (except `\n` if spans rely on newlines).

## Sentence‑Boundary Capping
- Prefer splitting on `. ! ?` followed by space/newline using a simple regex.
- If no boundary found before limit, cut hard at the nearest UTF‑8 char ≤ limit.

## Performance Notes
- Candidate cap limits memory (`MAX_CANDIDATES_PER_SOURCE`), independent of caller‑supplied `top_k`.
- Optional: add a small in‑process LRU cache for `content_hash` keyed by `(table,id,rev)` to avoid rehashing stable content in tight loops.

## Future Extensions (optional)
- Adaptive floor with small exploration budget; light RRF fusion (behind a flag).
- Recency/representativeness interleave strategy per source.
- Span offsets for precise UI highlighting.
- Per‑session Memory Pack helper (outside MCP server) to streamline synthesis agents.
