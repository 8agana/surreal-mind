# Phase C — Hypothesis Verification (Final step of refactor)

Owner: Codex • Date: 2025‑09‑07

Goal
- Add an optional, deterministic hypothesis verification loop to `legacymind_think` that gathers supporting/contradicting evidence from the Knowledge Graph (KG) and returns a confidence score and suggested revision when appropriate. No LLM calls are made in this phase; behavior is fully rule‑based and safe.

Scope
- Extend `legacymind_think` to accept an optional hypothesis and verification flag.
- Embed hypothesis → query KG (entities/observations) → classify evidence → compute confidence → return structured result.
- No changes to injection (still KG‑only). No DB schema changes required; verification result is returned in the tool response and optionally persisted as a JSON blob on the thought record if desired.

Non‑Goals
- No auto‑promotion to KG tables.
- No complex NLI or LLM reasoning; keep rules simple and explainable.

API Surface (legacymind_think additions)
- Args (additive, all optional):
  - `hypothesis?: string`
  - `needs_verification?: boolean` (default false)
  - `verify_top_k?: number` (default 100; max 500) — candidate pool size
  - `min_similarity?: number` (default 0.70; 0..1)
  - `evidence_limit?: number` (default 10; max 25) — per bucket
  - `contradiction_patterns?: string[]` (optional override; default list below)
- Result (additive):
  - `verification?: {
        hypothesis: string,
        supporting: EvidenceItem[],
        contradicting: EvidenceItem[],
        confidence_score: number,   // 0..1
        suggested_revision?: string,
        telemetry: {
          embedding_dim: number,
          provider: string,
          k: number,
          min_similarity: number,
          time_ms: number,
          matched_support: number,
          matched_contradict: number
        }
     }`
- EvidenceItem shape:
  - `{ table: "kg_entities"|"kg_observations", id: string, text: string, similarity: number, provenance?: any }`

Classification Rules (deterministic)
- Compute cosine similarity between hypothesis embedding and KG item embeddings (entities + observations). Keep items with `similarity >= min_similarity`.
- Contradiction detection (default patterns, case‑insensitive):
  - any of: ["not", "no", "cannot", "false", "incorrect", "fails", "broken", "doesn't", "isn't", "won't"] appearing near the core subject phrase.
  - Optional: if item has structured data with a boolean/polarity flag, treat `false/negative` as contradicting.
- Supporting = items above threshold that don’t match a contradiction pattern.
- Confidence score = `supporting.len() / max(1, supporting.len() + contradicting.len())`.
- Suggested revision: if `confidence_score < 0.4`, set to `"Consider revising hypothesis based on <n> contradicting items"`.

Implementation Steps
1) Schemas
   - Extend legacymind_think input/output schema with the fields above (minimum/maximum for numeric types).
2) Handler
   - If `hypothesis` present and `needs_verification=true` (or mode==debug/hypothesis trigger), run verification:
     - Embed hypothesis using the current embedder (dim‑safe).
     - Query KG entities/observations for nearest neighbors (respect existing dim filter):
       - Use existing vector search; candidate `k = verify_top_k`.
       - Filter to `similarity >= min_similarity`.
     - Classify items into supporting/contradicting via simple pattern rules.
     - Truncate to `evidence_limit` per bucket (highest similarity first).
     - Compute confidence and assemble telemetry.
   - Return result with `verification` block. Optionally, if a `persist_verification=true` internal flag is set (off by default), store a compact JSON under the thought (e.g., `verification: {...}`) without changing schema (SurrealDB permits dynamic fields).
3) Config (optional envs)
   - `SURR_VERIFY_TOPK` (default 100), `SURR_VERIFY_MIN_SIM` (default 0.70), `SURR_VERIFY_EVIDENCE_LIMIT` (default 10).
4) Testing
   - Unit: pattern matcher for contradictions; clamping and caps for k/limits; empty hypothesis → skip.
   - Integration (gated): with seeded KG, ensure items partition correctly and scores are computed as expected. Timing captured in telemetry.
5) Detailed Help
   - Add a section showing example usage for debug mode with hypothesis verification, and how to tune min_similarity and limits.

Acceptance Criteria
- With `hypothesis` + `needs_verification=true`, response includes a `verification` object with non‑empty telemetry.
- Evidence lists reflect rule‑based classification and respect `evidence_limit` caps.
- Confidence formula matches spec; suggested_revision appears when score < 0.4.
- No changes to core retrieval or injection behavior.
- Dimension/provider metadata present in telemetry; no mixed‑dim comparisons.

Risks / Mitigations
- False positives/negatives in contradiction detection → mitigated by transparent patterns, thresholds, and easy tuning of `min_similarity` and `contradiction_patterns`.
- Performance at large K → keep defaults modest (100) and cap to a sane max (500); short‑circuit early when sufficient evidence gathered.

Rollout
- Land schema + handler changes behind no feature flag (additive only).
- Announce to CC/Warp: pass hypothesis + needs_verification for debug scenarios; defaults suffice.
- Observe telemetry for 1–2 weeks; consider lifting to a standalone `verify_hypothesis` tool later if useful.

