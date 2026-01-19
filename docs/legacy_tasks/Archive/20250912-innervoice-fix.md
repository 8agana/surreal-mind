  Goal

  - Align inner_voice with think tools’ UX without persisting the user’s inquiry: persist only the synthesis and the feedback thoughts, linked for
  continuity. Keep KG auto-extraction tied to the synthesis thought.

  Behavior Changes

  - Two‑thought chain:
      - Thought A (persist): Synthesis result from Gemini CLI.
          - origin='inner_voice', injection_scale=0
          - previous_thought_id: pass-through from tool args if provided (do not create a query thought)
          - sources line + synth telemetry; no user query stored
      - Thought B (persist): Follow‑up/feedback prompt.
          - origin='inner_voice.feedback', injection_scale=0
          - previous_thought_id = ThoughtA
  - Tool return (no backward compatibility):
      - answer: exact synthesized text (returned inline and persisted)
      - synth_thought_id: ID of synthesis thought (primary identifier)
      - feedback: 1–3 line follow‑up prompt (also persisted)
      - feedback_thought_id: ID of feedback thought
      - sources_compact: compact citations line from selection
      - synth_provider, synth_model, embedding_dim
      - extracted: { entities, relationships }

  Entity Extraction Fixes (cause + fix)

  - Env mismatch (CLI extractor): Node script uses IV_CLI_; Rust synthesis uses IV_SYNTH_.
      - Fix: script accepts IV_CLI_* or falls back to IV_SYNTH_*; Rust also accepts both for extraction.
  - Bad default CLI args: “generate …” isn’t supported by Gemini CLI.
      - Fix: default args ["-m","{model}"] and read prompt from stdin.
  - JS syntax bug blocks Grok fallback (duplicate header key, missing comma).
      - Fix: correct headers in scripts/iv_extract.js.
  - Silent failures: Rust returns (0,0) without context.
      - Fix: log exit code and first 500 chars of stderr/stdout at debug; include provider and counts on success.
  - Optional safety net: if CLI and Grok both fail, call HeuristicExtractor on the synthesis text to stage a small capped set (entities ≤ 20, edges ≤ 30) with lower confidence.

  Implementation Steps

  - src/tools/inner_voice.rs
      - Remove any query‑thought persistence (do not insert the inquiry).
      - Persist synthesis thought; link to provided previous_thought_id (if any).
      - Persist feedback thought; link previous_thought_id to synthesis thought.
      - Response payload: add answer, synth_thought_id, feedback, feedback_thought_id, sources_compact, synth_provider, synth_model, embedding_dim, extracted counts; remove legacy/alias fields.
      - Extraction: continue staging with staged_by_thought = synth_thought_id.
      - Diagnostics: on CLI extractor non‑zero exit, log status, stderr/stdout snippet, and timing.
      - Optional: add include_feedback (default true) and feedback_max_lines (default 3).
  - scripts/iv_extract.js
      - Defaults: args ["-m","{model}"]; prefer IV_CLI_CMD/IV_CLI_ARGS_JSON; fallback to IV_SYNTH_*.
      - Fix callGrok headers; keep JSON repair + AJV validation.
  - lib/iv_utils.js and schemas/kg_extraction.schema.json
      - No structural change; ensure validator tolerates minimal outputs.
  - Docs
      - AGENTS.md/README/Brain files: two‑thought model (synthesis + feedback), continuity semantics, extractor env precedence (IV_CLI_* → IV_SYNTH_*), and log redaction (no user query persisted).
      - No Backward Compatibility: update references to use `synth_thought_id`; remove mentions of `thought_id` for inner_voice.

  Tests

  - E2E: inner_voice returns synth_thought_id and feedback_thought_id; DB contains exactly 2 new thoughts with correct origins and links; auto_extract
  staged rows reference synth_thought_id.
  - Failure path: simulate CLI failure → Grok fallback → stage >0; with Grok disabled → heuristic fallback → stage >0 for structured prompts.
  - No Backward Compatibility: do not expose `thought_id` alias; clients must use `synth_thought_id`.

  Acceptance Criteria

  - One call produces exactly two persisted thoughts (synthesis, feedback), linked in order, with no persisted user query.
  - Tool returns `answer` inline, plus `synth_thought_id`, `feedback`, `feedback_thought_id`, `sources_compact`, `synth_provider`, `synth_model`, `embedding_dim`, and `extracted` counts.
  - Auto‑extraction stages candidates with origin="inner_voice" and staged_by_thought = `synth_thought_id`.
  - Logs show a single INFO summary with { synth_id, feedback_id, provider, model, entities, edges, latency } and do not include user query content.
  - CLI extractor works with zero extra stdio config (uses sane defaults). Grok fallback and heuristic fallback both validated.

## Zed Clarification

General/Behavior Changes
1. **Feedback Thought Details**: The plan describes Thought B as a "Follow‑up/feedback prompt" with `origin='inner_voice.feedback'`. Is this prompt a fixed/static string (e.g., something like "Please provide feedback on this synthesis..."), or is it dynamically generated based on the synthesis content? If dynamic, what's the logic or source for generating it? Also, `feedback_text` in the tool return—does this refer to the full text of the persisted feedback prompt, or is it a separate output (e.g., a summary or user-facing message)?

2. **previous_thought_id Handling**: For Thought A (synthesis), it says to pass through `previous_thought_id` from tool args if provided. If not provided, should it be null/empty, or default to something? For Thought B, it's set to Thought A's ID. Any special handling if Thought A fails to persist (e.g., rollback or skip Thought B)?

3. **Tool Return Payload**: With no back-compat per Rules, we'll remove `thought_id` entirely and only include the new fields (`synth_thought_id`, `feedback_thought_id`, `feedback_text`, etc.). Does the plan intend to keep any other old fields, or is this a clean break?

### Entity Extraction Fixes
4. **Env Var Precedence**: The fix standardizes to accept `IV_CLI_*` or fallback to `IV_SYNTH_*`. Is there a priority order (e.g., always prefer `IV_CLI_*` if set, else `IV_SYNTH_*`)? Also, are there any other env vars involved in extraction that need similar handling?

5. **CLI Args and Stdin**: For the Gemini CLI, defaulting to `["-m","{model}"]` and reading the prompt from stdin—does `{model}` refer to a specific env var (e.g., `IV_CLI_MODEL` or something from the synthesis side)? And is the prompt (synthesis text) piped directly to stdin, or is there any formatting/wrapping?

6. **HeuristicExtractor Fallback**: This is mentioned as an optional safety net if both CLI and Grok fail. Is this already implemented in the codebase (e.g., in `lib/iv_utils.js` or elsewhere)? If not, do we need to implement it as part of this fix, or is it a separate component? What are the exact caps (entities ≤ 20, edges ≤ 30) and confidence scoring logic?

7. **JS Bug Fix**: Fixing headers in `scripts/iv_extract.js` for the Grok fallback—can you confirm the specific header issue (e.g., duplicate key or missing comma)? Is there a before/after example I can reference?

8. **Logging for Failures**: Adding debug logs for CLI exit code, stderr/stdout snippets, and timing—is there a specific log level or format (e.g., matching other tools like `legacymind_think`)? Also, for success, it mentions including provider and counts—does this go in the tool response or logs?

### Implementation Steps
9. **inner_voice.rs Changes**: Removing query-thought persistence and adding the two-thought persistence—any dependencies on existing structs/schemas (e.g., in `src/schemas.rs`)? For the optional `include_feedback` and `feedback_max_lines`, what does `feedback_max_lines` control (e.g., truncating `feedback_text` to N lines)? If these are optional, should they be added as tool parameters now, or gated behind an env var?

10. **Scripts and Libs**: Updates to `scripts/iv_extract.js`, `lib/iv_utils.js`, and `schemas/kg_extraction.schema.json`—are there any version bumps or compatibility checks needed (e.g., for the JSON schema)? Also, ensuring the validator tolerates minimal outputs—does "minimal" mean empty arrays or just fewer fields?

11. **Docs Updates**: Updating AGENTS.md/README for the two-thought model, continuity fields, and env precedence—are there specific sections to update (e.g., the Inner Voice subsection)? Any examples or screenshots to include?

### Tests and Acceptance
12. **E2E Tests**: The plan outlines tests for the two thoughts, links, and staging. Do we need to add these to `tests/` or extend existing ones? Any specific mock setups for CLI/Grok failures?

13. **Failure Paths**: Simulating CLI failure → Grok → heuristic—how do we simulate these in tests (e.g., env vars to force failures)? Also, ensuring >0 staged items for structured prompts—what constitutes "structured"?

## Zed Q&A — Answers (Decisions)

1) Feedback Thought Details
- Dynamic generation via Gemini CLI based on the synthesized answer. Template: “Propose the single highest‑impact next question that would improve the answer above. Keep it under 2 short lines. No bullets, no preamble.” Temperature 0.2.
- `feedback_text` returned by the tool is exactly the persisted content of the feedback thought (no separate summary).

2) previous_thought_id Handling
- Synthesis (Thought A): if `previous_thought_id` is provided in tool args, set it; otherwise leave null/absent. Do not invent a default.
- Feedback (Thought B): always set `previous_thought_id = synth_thought_id`.
- Failure policy: if persisting Thought A fails, abort the tool with a structured error `PersistenceError.Synthesis`; do not create Thought B and do not run extraction.

3) Tool Return Payload (Clean Break)
- No legacy fields or aliases. Do not return `thought_id`.
- Definitive fields: `answer`, `synth_thought_id`, `feedback`, `feedback_thought_id`, `sources_compact`, `synth_provider`, `synth_model`, `embedding_dim`, `extracted`.

4) Env Var Precedence (Extraction)
- Priority: `IV_CLI_CMD`/`IV_CLI_ARGS_JSON`/`IV_MODELS` → fallback to `IV_SYNTH_CLI_CMD`/`IV_SYNTH_CLI_ARGS_JSON`/`GEMINI_MODEL` → defaults (`gemini`, args `["-m","{model}"]`, model `gemini-2.5-pro`).
- Fallback provider: `IV_ALLOW_GROK` (default true), `GROK_API_KEY`, `GROK_MODEL`, `GROK_BASE_URL`.

5) CLI Args and Stdin
- `{model}` resolves to the first of `IV_MODELS` (if present) else `GEMINI_MODEL` else `gemini-2.5-pro`.
- Prompt delivery: via stdin. For synthesis (Rust), the prompt uses `build_cli_prompt(query, snippets)`; for extraction (Node), `lib/iv_utils.buildPrompt()` provides a JSON‑schema instruction with the synthesized text.

6) HeuristicExtractor Fallback
- Not currently invoked by inner_voice; implement as last resort if CLI and Grok both fail.
- Caps: up to 20 entities and 30 edges after dedupe. Confidence: entities 0.7 default; edges 0.6 default.
- Staging: origin="inner_voice", status="pending", `staged_by_thought = synth_thought_id`.
- Toggle: enabled by default; can be disabled with `SURR_IV_HEURISTIC_FALLBACK=0`. Optional caps via `SURR_IV_HEURISTIC_MAX_ENTITIES`/`SURR_IV_HEURISTIC_MAX_EDGES`.

7) JS Bug Fix (Grok Fallback)
- Issue: duplicate `'Content-Type'` header and missing comma caused syntax/runtime error.
- Before:
  ```js
  headers: {
    'Authorization': `Bearer ${key}`,
    'Content-Type': 'application/json'
    'Content-Type': 'application/json'
  },
  ```
- After:
  ```js
  headers: {
    'Authorization': `Bearer ${key}`,
    'Content-Type': 'application/json'
  },
  ```

8) Logging for Failures/Success
- Failure (DEBUG): `inner_voice.extract_fail { cmd, code, stderr_snip, stdout_snip, latency_ms }` (snippets truncated to 500 chars each).
- Success (INFO): `inner_voice.extract { synth_id, provider, model, entities, edges, latency_ms }`.
- Only counts/ids in logs; no user query text is logged.

9) inner_voice.rs Changes and Params
- Schema dependency: update `src/schemas.rs` inner_voice return shape to include the new fields; add optional params:
  - `include_feedback` (bool, default true)
  - `feedback_max_lines` (int, default 3; truncate feedback_text to N lines on newline boundaries)
- Parameters are tool args (not env) for explicit per‑call control.

10) Scripts/Libs Compatibility
- No version bump required. `schemas/kg_extraction.schema.json` already tolerates minimal objects.
- “Minimal outputs” means `entities: []`, `edges: []`, optional `doc_meta`. The validator should accept empty arrays.

11) Docs Updates
- Update AGENTS.md sections: “Inner Voice”, “Extractor env precedence”, and tool response examples.
- Brain files: note two‑thought model (synthesis + feedback) and that the inquiry is not persisted.

12) E2E Tests
- Add tests under `tests/inner_voice_flow.rs`:
  - Happy path: assert 2 thoughts persisted with correct origins/links; response returns `answer` and thought IDs; extraction staged rows reference `synth_thought_id`.
  - CLI fail → Grok pass: set `IV_CLI_CMD=/bin/false`, keep valid `GROK_API_KEY`.
  - CLI fail → Grok disabled → heuristic: `IV_CLI_CMD=/bin/false`, `IV_ALLOW_GROK=false`.
- Fixture/mocks: allow overriding `IV_SCRIPT_PATH` to a fixture that prints valid JSON to test the Node path deterministically.

13) Failure Path Simulation and “Structured” Prompts
- Simulate failures via env as above; enforce >0 staging by using prompts with explicit relational cues, e.g., “inner_voice stages_to memories_moderate”, “A uses B”, “X depends on Y”, or “A -> B”. These satisfy the extractor’s simple patterns.
