# think_convo Frameworks v1 — Local, Deterministic “Alive” Thinking

Status: Draft (ready for implementation)
Owner: Grok (implementation), Codex (design)
Last updated: 2025-09-06

## Context

We want think_convo to feel alive without being a chat or requiring any external API. The tool stores a thought, then applies a fast, local “thinking enhancement” pass that nudges perspective and next action. No essays, only compact, structured output saved on the thought.

This document specifies a small, deterministic engine with three layers inspired by OODA that runs locally in ≤600 ms and produces a strict JSON envelope stored in `framework_analysis` with `framework_version = "convo/1"`.

## Goals
- Local-only: zero network calls, no model dependency.
- Deterministic: same input → same envelope (seeded variety allowed via stable hashing).
- Helpful, concise output: 2–3 takeaways, ≤2 prompts, one concrete next step.
- Safe and fail-open: thoughts always persist; enhancement may skip on error/timeout.

## Non-Goals
- No multi-turn sessions in v1 (no conversation loop).
- No long-form generation; no external LLMs.

## High-Level Design

Three-layer engine applied to the stored thought content:

1) Sensorium (Observe/Orient)
- Compute quick heuristics:
  - intent_polarity: `explore|decide|vent` (keyword/verb heuristics)
  - valence: `positive|negative|neutral` (tiny lexicon)
  - complexity: `low|med|high` (length, type–token ratio, causal/constraint token counts)
  - stalled: bool (very short/empty, filler-only, or too diffuse)

2) Strategy Core (Decide)
- Choose a methodology based on heuristics (priority order):
  - `mirroring` if vent ∧ negative
  - `first_principles` if decide ∧ (complexity=high ∨ has causal tokens)
  - `socratic` if explore ∧ complexity=high
  - `lateral` if stalled
  - else `constraints`

3) Response Generation (Act)
- Produce a strict JSON envelope:

```json
{
  "framework_version": "convo/1",
  "methodology": "socratic | first_principles | mirroring | lateral | constraints",
  "data": {
    "summary": "string",
    "takeaways": ["string"],
    "prompts": ["string"],
    "next_step": "string",
    "tags": ["string"]
  }
}
```

- Deterministic template selection is seeded by `blake3(content_norm)` so outputs feel varied but stable.

## Data Model & Schema

- Thought fields already exist (confirmed in current schema init on master):
  - `framework_enhanced: option<bool>`
  - `framework_analysis: option<object>`
- This feature writes:
  - `framework_enhanced = true` on success (false/None on failure)
  - `framework_analysis = FrameworkEnvelope` (above)
- Versioning: always include `framework_version: "convo/1"`.

Schema check: If running against a new DB, ensure schema init defines these fields on `thoughts`. If not present, add them in the schema init block before enabling this feature.

## Config & Controls

Environment (with defaults for think_convo only):
- `SURR_THINK_ENHANCE=1` — enable enhancement (default ON for think_convo)
- `SURR_THINK_ENHANCE_TIMEOUT_MS=600` — hard timeout; on timeout → skip
- `SURR_THINK_STRICT_JSON=1` — if envelope fails validation → drop (fail-open)
- `SURR_THINK_TAG_WHITELIST=plan,debug,dx,photography,idea` — allowed tags to merge

Lexicon overrides (optional; comma‑separated, lower‑case):
- `SURR_THINK_LEXICON_DECIDE="decide,ship,fix,choose,implement,deploy,select,finalize"`
- `SURR_THINK_LEXICON_VENT="hate,pissed,broken,fuck,shit,sucks,awful"`
- `SURR_THINK_LEXICON_CAUSAL="because,why,root,reason,due,constraint,risk,block,cause"`

Per-call (existing):
- `verbose_analysis: bool` — treat as override ON for this call.

## Module Layout

```
src/
  frameworks/
    mod.rs            // entry point: run_convo(content, opts) -> FrameworkEnvelope
    convo.rs          // heuristics, methodology selection, generators
```

Public API (Rust):
```rust
pub struct FrameworkEnvelope<T> { pub framework_version: String, pub methodology: String, pub data: T }

pub struct ConvoData {
  pub summary: String,
  pub takeaways: Vec<String>,
  pub prompts: Vec<String>,
  pub next_step: String,
  pub tags: Vec<String>,
}

pub struct ConvoOpts { pub strict_json: bool, pub tag_whitelist: Vec<String>, pub timeout_ms: u64 }

pub fn run_convo(content: &str, opts: &ConvoOpts) -> anyhow::Result<FrameworkEnvelope<ConvoData>>;
```

Integration point:
- In `src/tools/convo_think.rs`, after inserting the thought and before KG injection:
  - if `SURR_THINK_ENHANCE=1` or `verbose_analysis==true`: call `frameworks::run_convo` with timeout; on Ok → set fields; on Err/timeout → set `framework_enhanced=false` and continue.

## Sensorium — Heuristics (deterministic)

Normalization: lower-case, strip extra whitespace.

- intent_polarity:
  - decide if contains verbs like: decide, ship, fix, choose, implement; or decision markers ("should", "need to" + action)
  - vent if contains strong negative words (awful, hate, broken, pissed, fuck) and exclamations
  - else explore
- valence:
  - tiny lexicon: positive=[great, good, love, nice, excited], negative=[bad, broken, hate, stuck, fuck, shit]
  - score = positives - negatives → bucket: pos/neg/neutral
- complexity:
  - tokens = split on whitespace; unique_ratio = unique/tokens
  - causal tokens: because, why, cause, root, reason, due, constraint, risk
  - if tokens>60 or (unique_ratio>0.6 and causal_count>=2) → high; else if tokens>25 → med; else low
- stalled:
  - true if tokens<6 OR valence-neutral AND tokens<10

Expose these in a `ConvoSense` struct for unit tests.

## Strategy Core — Methodology Selection

Priority evaluation:
1. if intent=vent and valence=negative → `mirroring`
2. else if intent=decide and (complexity=high or causal_count>=2) → `first_principles`
3. else if intent=explore and complexity=high → `socratic`
4. else if stalled → `lateral`
5. else `constraints`

Include the chosen `methodology` in the envelope.

## Response Generation — Templates (deterministic)

Determinism & seeding: derive a stable u64 from `blake3(content_norm)` using the first 8 bytes, then `idx = seed % N` to pick among 2–3 short phrasings per slot. Add a unit test asserting stability for a fixed input.

Shared helpers:
- `one_line_summary(content)`: truncate/clean to ~100 chars; prefer root verb + object if present.
- `safe_next_step(methodology)`: a single concrete action tailored to methodology.
- `tag_merge(proposed)`: intersect with whitelist.

Methodologies (each returns `ConvoData`):

1) Socratic (`"socratic"`)
- takeaways (≤2): assumption to test, term to define
- prompts (≤2): "What would make this obviously wrong?", "Which constraint matters most?"
- next_step: write one crisp question and answer it in 2–3 lines
- tags: ["plan","dx"]

2) First Principles (`"first_principles"`)
- takeaways: problem reduction in one clause; key variable to measure
- prompts: "If you removed X, what remains?"
- next_step: list 2 primitives that determine outcome; choose one to instrument
- tags: ["debug","plan"]

3) Empathetic Mirroring (`"mirroring"`)
- takeaways: one-line reflection of feeling; one stabilizer (boundary/breath/step away)
- prompts: "What would feeling 10% better look like?"
- next_step: the smallest stabilizing action (5–10 min)
- tags: ["convo"]

4) Lateral (`"lateral"`)
- takeaways: adjacent concept/analogy; constraint you can borrow from another domain
- prompts: "What is the opposite lever?"
- next_step: try a tiny experiment inspired by the analogy
- tags: ["idea"]

5) Constraints & Levers (`"constraints"`)
- takeaways: top constraint; top lever
- prompts: "What single change increases the lever?"
- next_step: apply the lever once in a 15‑min block
- tags: ["plan"]

## Validation & Strict Mode

- Build a serde struct for `FrameworkEnvelope<ConvoData>` and validate lengths:
  - takeaways ≤ 2; prompts ≤ 2; next_step ≤ 140 chars; summary ≤ 140 chars (hard truncate summary to 140 prior to validation)
- If `SURR_THINK_STRICT_JSON=1` and validation fails → set `framework_enhanced=false` and skip storing `framework_analysis`.

## Telemetry (respect MCP_NO_LOG)

Counters (increment as info/debug when logging enabled):
- `think.convo.enhance.calls`
- `think.convo.enhance.timeout`
- `think.convo.methodology.{name}`
- `think.convo.enhance.drop_json`
- `think.convo.finalize.ms`

## Integration Steps

1) Add `frameworks` module and convo implementation.
2) Wire `convo_think` handler:
   - After CREATE thought, call `frameworks::run_convo` with timeout.
   - On Ok → `framework_enhanced=true; framework_analysis=envelope` via `UPDATE thoughts`.
   - On Err/timeout → `framework_enhanced=false` (do not fail the tool).
3) Keep KG injection unchanged (runs after enhancement).

## Tests

Unit:
- Sensorium classification cases (intent/valence/complexity/stalled).
- Methodology selection priority.
- Determinism: same content → same envelope.
- Validation: envelopes exceeding limits are dropped in strict mode.
 - Neutral/low-signal input routes to `constraints`.
 - Seeding stability: same normalized input → same template index.

E2E (small):
- Happy path: think_convo with a medium complex decision utterance → `first_principles` chosen; fields populated; injection still runs.
- Venting: negative vent → `mirroring` chosen; fields populated.

## Performance & Safety
- Pure string ops (O(n)); target overhead ≤ 200–300 ms.
- No network calls; respects `MCP_NO_LOG`.
- No schema changes required if `framework_enhanced` and `framework_analysis` already exist. If not, ensure schema init defines them as option fields on `thoughts`.

## Rollout
- Default ON for `think_convo` (`SURR_THINK_ENHANCE=1`); keep other think_* unchanged for now.
- If issues, disable with `SURR_THINK_ENHANCE=0`.

## Operational Playbook

- Observe in logs (when MCP_NO_LOG is off):
  - `think.convo.enhance.timeout` — spikes indicate rising cost; consider disabling.
  - `think.convo.enhance.drop_json` — schema violations; inspect inputs, tune limits/lexicons.
  - `think.convo.methodology.{name}` — distribution sanity check (mirroring shouldn’t dominate).
  - `think.convo.finalize.ms` — keep p50 ≈ 200–300ms, well under 600ms timeout.
- Rollback: set `SURR_THINK_ENHANCE=0` to disable enhancement while leaving storage/injection intact.

## Deliverables Checklist
- [ ] `src/frameworks/mod.rs` + `src/frameworks/convo.rs` with public API
- [ ] `convo_think.rs` integration with timeout and fail-open path
- [ ] Validation + limits + strict mode handling
- [ ] Unit tests for heuristics/selection/determinism
- [ ] E2E test for one happy path
- [ ] Minimal telemetry hooks (guarded by logs)

## Pseudocode (convo)

```rust
fn run_convo(content: &str, opts: &ConvoOpts) -> Result<FrameworkEnvelope<ConvoData>> {
  let norm = normalize(content);
  let sense = analyze(norm);
  let method = select_methodology(&sense);
  let seed = blake3_32(norm.as_bytes());
  let data = generate_convo_data(method, &norm, seed, &opts.tag_whitelist);
  validate(&data, opts.strict_json)?;
  Ok(FrameworkEnvelope { framework_version: "convo/1".into(), methodology: method.to_string(), data })
}
```

---

Appendix: Keyword Sets (initial)
- decide: [decide, ship, fix, choose, implement, deploy, select, finalize]
- vent: [hate, pissed, broken, fuck, shit, sucks, awful]
- causal/constraint: [because, why, root, reason, due, constraint, risk, block, cause]
