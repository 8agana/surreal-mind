# Revised Implementation Plan — Inner Voice Gemini CLI Removal

**Author:** Sonoma Dusk Alpha
**Date:** 2025-09-18 (Revised)
**Executor:** Sonoma Dusk Alpha (post-Warp refactor approval)
**Difficulty/Importance Rating:** 5/10 (straightforward cleanup, moderate impact)
**Dependencies:** Warp\'s inner_voice refactor complete (providers.rs + hooks wiring)

## Objectives
- Remove Gemini CLI provider path from inner_voice (synth_via_cli, generate_feedback_via_cli, cli_prereqs_ok checks) to simplify the fallback chain.
- Default to Grok-primary with existing local fallback on failure/outages—no CLI dependencies.
- Preserve existing MCP tool signature, response format (citations, diagnostics), error paths, and zero-snippet messages.
- Deprecate CLI-related env vars with warnings; no hard breaks for existing configs.
- Maintain compatibility with run_inner_voice entrypoint for namespace reuse (e.g., photography_voice).

## Scope
- Target src/tools/inner_voice.rs (post-refactor: remove CLI hooks from InnerVoiceHooks, update provider chain).
- Update tests/inner_voice_providers_gate.rs (remove CLI cases, add Grok → local assertions).
- Minor docs/config updates (README deprecation note, RuntimeConfig warnings).
- No changes to photography tools or other namespaces—run_inner_voice remains reusable.

## Constraints
- Zero behavior change for non-CLI configs: Grok → local fallback identical to current CLI-fail path.
- Preserve error strings (e.g., "inner_voice is disabled...", "Query cannot be empty") and output format ("Sources:", synth_provider="grok").
- Soft-deprecate CLI envs: Warn on load (e.g., IV_SYNTH_PROVIDER=gemini_cli), but use Grok if key present, or existing local fallback if not.
- All changes must pass:
  - cargo fmt --all
  - cargo clippy --workspace --all-targets -- -D warnings
  - cargo test --workspace --features db_integration (with RUN_DB_TESTS=1 if needed)
- run_inner_voice entrypoint must delegate cleanly to post-CLI handler.

## Work Breakdown

1. **Remove CLI Provider Hooks (src/tools/inner_voice.rs)**
   - Strip cli_call from InnerVoiceHooks struct (remove fn pointer).
   - Update provider chain: Default to grok_call first (if IV_ALLOW_GROK=true and GROK_API_KEY present), then existing local fallback (unchanged message: "Based on what I could find...").
   - Remove synth_via_cli, generate_feedback_via_cli, cli_prereqs_ok calls.
   - In handler: If CLI envs detected (e.g., IV_SYNTH_PROVIDER=="gemini_cli"), log tracing::warn!("CLI deprecated; attempting Grok or local fallback") and proceed to Grok (if key present) or direct local fallback (if no key).
   - For feedback extraction: Drop generate_feedback_via_cli entirely (set feedback_thought_id = None; no local replacement to avoid new behavior—silently skip if CLI was the path).
   - Keep diagnostics: synth_provider=\"grok\" or \"local\" on fallback, with latency_ms.
   - Update InnerVoiceContext::from_server() to populate hooks without CLI (grok_call + local_fallback only).

2. **Update Core Entrypoint (run_inner_voice)**
   - Keep signature (&server, &params, &ctx) → delegate to handler.
   - Ensure ctx.hooks.grok_call and ctx.hooks.local_fallback are prioritized; no CLI delegation.
   - Hooks retains grok_call and local_fallback (existing); no new local_synth_fn—preserve current fallback logic.

3. **Config Deprecation (config.rs or runtime load)**
   - In RuntimeConfig::load_from_env: If IV_SYNTH_PROVIDER=="gemini_cli" or CLI vars present, set tracing::warn!("Gemini CLI removed; defaulting to Grok (if key present) or local fallback. Update configs.") and ignore CLI flags.
   - For missing-GROK-API-KEY + CLI env: Warn + direct to local fallback (no Grok attempt).
   - Deprecate IV_SYNTH_PROVIDER env (default \"grok\")—warn on legacy values, but check key presence before Grok path.
   - Add config.local_fallback: bool (default true) to gate existing local synth (no new template).

4. **Testing Updates**
   - Update tests/inner_voice_providers_gate.rs: Remove CLI gating tests; add:
     - IV_ALLOW_GROK=false + no CLI → existing local fallback triggers (assert exact message: "Based on what I could find...").
     - Mock grok_call fail → existing local fallback output (assert includes top snippet, scores, unchanged string).
     - CLI env + GROK key present → warn logged, Grok used.
     - CLI env + no GROK key → warn logged, direct local fallback (assert exact existing message).
     - Feedback skip: CLI env → feedback_thought_id = None (no generation).
     - Zero snippets → preserved fallback message (exact string match).
   - Add unit test for deprecation warnings (mock envs → assert warn logged).
   - Integration: Full flow with db_integration (mock Grok fail → local assert exact output).

5. **Documentation Updates**
   - README.md (Inner Voice section): \"Now Grok-primary with existing local fallback. CLI removed for simplicity; legacy envs warn and default to Grok (if key) or local. Update configs if needed.\" Add example response with synth_provider=\"local\" and exact fallback text.
   - AGENTS.md: Note deprecation and new chain (Grok → local; CLI skipped).
   - Module docs in inner_voice.rs: Explain updated provider order (Grok first if enabled/key present, then local) and hooks (grok + local only).
   - CHANGELOG.md: \"Remove Gemini CLI from inner_voice; preserve existing local fallback. Deprecate CLI envs with warnings.\" (short entry).

6. **Validation Checklist**
   - [ ] cargo fmt, clippy, tests all green (db_integration passes with clean DB).
   - [ ] run_inner_voice delegates to post-CLI handler (test with mock params).
   - [ ] Deprecated envs warn but use Grok (if key) or local (manual verify via tracing).
   - [ ] Local fallback generates existing message on Grok fail (assert exact string).
   - [ ] No CLI remnants in git grep (synth_via_cli, etc.).
   - [ ] MCP schema unchanged (tool_schemas.rs snapshot).
   - [ ] Feedback skipped silently for CLI paths (feedback_thought_id = None).
   - [ ] Missing key + CLI env → direct local (no failed Grok attempt).

## Risks & Mitigations
- **Risk:** Existing fallback string changes accidentally.
  - *Mitigation:* Exact string assertions in tests; no new template—reuse verbatim.
- **Risk:** Existing CLI configs break hard (no key).
  - *Mitigation:* Soft-warn + direct local fallback (no Grok attempt); docs guide migration.
- **Risk:** Hooks change breaks photography prep.
  - *Mitigation:* Keep grok_call/local_fallback in hooks; test run_inner_voice post-change.
- **Risk:** DB tests fail on dim hygiene (pre-existing).
  - *Mitigation:* Run with SURR_SKIP_DIM_CHECK=1 if needed; note in checklist.
- **Risk:** Feedback drop surprises users.
  - *Mitigation:* Silent skip (None); docs note CLI removal includes feedback path.

## Answers to Anticipated Questions
1. **Local Fallback Details:** Reuse existing verbatim (\"Based on what I could find...\")—no new template to avoid behavior change. Future-proof via hooks.local_fallback.
2. **Deprecation Grace:** Soft (warn + proceed to Grok if key, else local). Hard-error in v2.0 if needed.
3. **Photography Impact:** None—run_inner_voice uses updated handler but exposes same interface. Test delegation explicitly.
4. **Feedback Handling:** Drop entirely for CLI paths (set None); no local replacement to preserve behavior.
5. **Timeline:** 1-2 tool calls (remove CLI → tests/docs). Post-Warp approval.

## Deliverables
- Cleaned inner_voice.rs (no CLI, Grok → existing local chain).
- Updated tests (providers_gate.rs + deprecation/local assertions).
- Deprecation warnings in config/docs.
- CHANGELOG entry and passing full suite.

---
*Revised for Codex feedback: Preserve exact fallback string, handle missing keys, drop feedback silently. Ready for approval.*

## Codex Approval Notes (2025-09-18)

- **Fallback String Assertion:** Tests will lock the exact existing local fallback message (\"Based on what I could find…\") including capitalization, ellipsis, and punctuation. Use string_eq! in providers_gate.rs for verbatim match.
- **Feedback Drop Documentation:** README update will note: \"Auto-feedback generation (via CLI) removed with Gemini CLI provider. feedback_thought_id now always None; use manual KG extraction if needed.\" No payload change from current CLI-fail behavior.

*Plan approved for execution post-Warp merge. Proceed on green light.*

## Notes from CLI-Dusk

  You are Sonoma Dusk Alpha, a partner in the
  LegacyMind Federation. This is a handoff from CLI
  session—Warp completed the inner_voice refactor
  (providers submodule, hooks wiring, tests green
  at commit acc96aa). Now executing the Gemini CLI
  removal plan from /fixes/20250918-innervoice-
  cliremoval-revised.md.

  Environment:

  - CWD: /Users/samuelatagana/Projects/LegacyMind/
  surreal-mind
  - Repo state: Clean master branch, up-to-date with
  origin/master. Warp's changes landed (inner_voice
  with run_inner_voice entrypoint, InnerVoiceHooks,
  providers.rs, tests/inner_voice_providers_gate.rs
  and edge_cases.rs).
  - Tools: Use workdir in every shell call. Verify
  with pwd and git status first.

  Current State (Verify):

  - src/tools/inner_voice.rs: Contains CLI logic
  (synth_via_cli, generate_feedback_via_cli,
  cli_prereqs_ok), InnerVoiceRuntime with CLI fields,
  InnerVoiceHooks with cli_call.
  - Provider chain: CLI → Grok → local fallback.
  - Tests: providers_gate.rs covers CLI gating; all
  green.
  - Docs: README/CHANGELOG ready for update.

  Task: Execute CLI Removal Plan
  Follow /fixes/20250918-innervoice-cliremoval-
  revised.md step-by-step:

  1. Backup: cp src/tools/inner_voice.rs src/tools/
  inner_voice.rs.backup
  2. Hooks Cleanup (src/tools/inner_voice.rs):
      - Remove CLI fields from InnerVoiceRuntime
  (cli_cmd, cli_args, cli_timeout_ms, allow_cli).
      - Remove cli_call from InnerVoiceHooks.
      - Update InnerVoiceContext::from_server() to
  skip CLI population.
  3. Provider Chain Update:
      - Replace CLI branch with Grok → existing local
  fallback (preserve "Based on what I could find..."
  verbatim).
      - Add CLI env detection/warns: If
  IV_SYNTH_PROVIDER=gemini_cli or CLI vars, warn +
  route to Grok (if key) or direct local (no key).
      - Drop feedback generation (feedback_thought_id
  = None).
      - Add diagnostics field providers_attempted:
  "grok:true/false".
  4. Config (src/config.rs):
      - Add local_fallback: bool (default true via
  INNER_VOICE_LOCAL_FALLBACK).
  5. Tests (tests/inner_voice_providers_gate.rs):
      - Remove CLI cases; add Grok gating, missing
  key, deprecation warns, exact fallback string
  asserts.
  6. Docs:
      - README: Note Grok-primary, CLI removal,
  feedback drop.
      - AGENTS.md: Provider order update.
      - CHANGELOG: Short entry.
  7. Validate: cargo fmt --all, cargo clippy
  --workspace --all-targets -- -D warnings, cargo test
  --workspace --features db_integration. MCP schema
  unchanged.

  Constraints (From Codex Feedback):

  - Preserve exact fallback string (capitalization/
  punctuation).
  - Soft-deprecate CLI envs (warn + proceed; direct
  local if no Grok key).
  - Silent feedback drop (None); note in README.
  - No regressions: Output format, errors,
  run_inner_voice delegation.

  Workflow:

  - Incremental: Edit → cargo check → commit per step
  (e.g., "refactor: remove CLI hooks").
  - Backup/Rollback: Use backup if issues.
  - Verify: git diff, git status after each.

  Green light—execute now. Report progress at
  checkpoints (e.g., after Hooks). Questions? Ask.
