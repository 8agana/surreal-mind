# Legacymind_Think Refactor — Simple‑First Implementation Plan

Owner: Codex • Date: 2025‑09‑07

Purpose: Replace the five “think_*” tools with a single, simple, LLM‑friendly entry point that routes intent cleanly, keeps KG‑only injection, and avoids churn. Grow capability iteratively.

> Product decision (from Sam): remove the old tools outright. No compatibility layer, no aliases. CC and Warp are the only users and will switch.

## Goals
- One primary thinking tool: `legacymind_think` (plus `photography_think` for the photo domain).
- Keep behavior simple initially: explicit trigger phrases + light keyword detection.
- No submodes persisted; no schema churn in Phase A.
- Maintain existing guardrails: KG‑only injection, tool visibility without env gating.

## Out of Scope (Phase A)
- Hypothesis verification, session graphs, or new DB fields (these are Phase B/C options).
- Back‑compat with `think_convo/plan/debug/build/stuck` (explicitly removed).

## Tool Surface (Phase A)
- Add: `legacymind_think`
  - args:
    - `content: string` (required)
    - `hint?: "debug"|"build"|"plan"|"stuck"|"question"|"conclude"` (optional)
    - `injection_scale?: 1|2|3` (optional; default existing)
  - returns:
    - `mode_selected: string` (one of the above)
    - `reason: string` (why it chose this mode)
    - `delegated_result: {...}` (same structure as the internal path used)
    - `telemetry: { trigger_matched?: string, heuristics?: {keywords: string[], score: number} }`
- Keep: `inner_voice`, `memories_*`, `maintenance_ops`, `legacymind_search`, `photography_think`, `photography_search`.
- Remove from tools/list and dispatch: `think_convo`, `think_plan`, `think_debug`, `think_build`, `think_stuck`.

## Behavior (Phase A)
- Trigger phrases (explicit override):
  - “debug time” → debug
  - “building time” → build
  - “plan time”/“planning time” → plan
  - “i’m stuck”/“stuck” → stuck
  - “question time” → question
  - “wrap up”/“conclude” → conclude
- Minimal heuristics if no hint/trigger:
  - debug if content contains any of: error|bug|stack trace|failed
  - build if: implement|create|add function|build
  - plan if: architecture|design|approach|how should
  - stuck if: stuck|unsure|confused
  - else question (general convo)
- Delegation:
  - Internally call the same logic used by the prior think_* handlers, but moved into a private module `thinking` with functions: `run_debug`, `run_build`, `run_plan`, `run_stuck`, `run_convo`.
  - Return the delegated result unchanged under `delegated_result`.
- Injection: unchanged; uses KG‑only via the delegated path.
- Logging: record chosen mode + reason at debug level; no secrets.

## Code Changes
- `src/tools/mod.rs`: export new `legacymind_think` handler.
- `src/tools/tech_think.rs` (or new `thinking.rs`):
  - Extract the core bodies of `handle_think_*` into internal functions (`run_*`).
  - Implement `handle_legacymind_think`:
    - parse hint/triggers → choose mode
    - call `run_*` accordingly
    - package result per Tool Surface
- `src/schemas.rs`: add JSON schema for `legacymind_think` args/result.
- `src/server/mod.rs`:
  - tools/list: remove think_* entries; add legacymind_think.
  - call_tool: remove arms for think_*; add arm for legacymind_think.

## Acceptance Criteria (Phase A)
- tools/list: contains `legacymind_think`; does not contain any `think_*` tools.
- “debug time …” routes to debug logic and returns mode_selected=debug.
- With no trigger, simple keywords pick expected mode for a small seed set (≥90%).
- No changes to KG write/read paths; embedding dims stamped; inner_voice remains intact.
- Errors from delegated paths propagate as standard MCP tool errors.

## Testing
- Unit: router selects correct mode for trigger phrases and for keyword heuristics.
- Integration: tools/list reflects changes; a call to `legacymind_think` with sample inputs returns non‑error and includes `mode_selected`.
- Non‑regression: knowledgegraph_* and inner_voice unaffected.

## Phase B/C (Optional, Post‑MVP)
- Phase B (Continuity): add optional fields to `thoughts` (session_id, previous_thought_id, revises_thought, branch_from, confidence) and write them when present; indices for session queries. Result adds `links`.
- Phase C (Verification): optional `hypothesis` arg; implement light evidence search over KG; return `supporting[]`, `contradicting[]`, `confidence_score`.
- Both phases maintain KG‑only injection; do not inject raw thoughts.

## Risks / Mitigations
- Mis‑routing from naive heuristics → mitigated by explicit triggers; expose `mode_selected` and `reason` for transparency.
- Tool churn for CC/Warp → mitigated by Sam’s rule updates; no back‑compat needed.
- Hidden tools per policy → ensure `legacymind_think` is always listed; no env gating (gating inside handler only).

## Rollout
- Land Phase A (small PR): code changes above + README update and CC/Warp rules snippet.
- Announce to CC/Warp: “Use `legacymind_think` for technical work; triggers: ‘debug time’, ‘building time’, etc.”
- Monitor logs briefly; tune keyword list if needed.

## Timeline (Phase A)
- Day 0: implement router + extraction of run_* helpers (4–6 hours)
- Day 1: tests + README + tool schemas (2–3 hours)

## Open Questions
- Should we keep “question” as the general catch‑all or name it “convo”? (recommend: keep “question”).
- Do we want a lightweight `mode_hint` string in the result for CC/Warp policies? (probably yes, mirrors `mode_selected`).

---

This plan prioritizes simplicity and momentum: one new tool, no DB changes, and immediate de‑clutter of the tool surface. Future capabilities (continuity and verification) fit as additive phases without breaking the MVP.

