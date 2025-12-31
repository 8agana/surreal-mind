# Implementation Plan: Remove inner_voice Tool (Planning Only)

## Scope
- Goal: remove inner_voice tool and all supporting code, tests, scripts, and docs with no dead code remaining.
- This document is plan-only; no code changes are executed here.

## Inventory: files referencing inner_voice
### Runtime / source
- src/tools/inner_voice.rs
- src/tools/inner_voice/providers.rs
- src/tools/mod.rs
- src/server/router.rs
- src/schemas.rs
- src/tools/detailed_help.rs
- src/tools/maintenance.rs
- src/config.rs
- src/main.rs

### Tests
- tests/inner_voice_retrieve.rs
- tests/inner_voice_edge_cases.rs
- tests/inner_voice_providers_gate.rs
- tests/inner_voice_flow.rs
- tests/tool_schemas.rs

### Scripts / libs
- scripts/iv_extract.js
- lib/iv_utils.js
- scripts/package.json

### Docs / notes / prompts
- README.md
- GEMINI.md
- CHANGELOG.md
- docs/AGENTS/tools.md
- docs/AGENTS/arch.md
- docs/AGENTS/roadmap.md
- docs/AGENTS/todo.md
- docs/troubleshooting/20251221-20251224-memories_populate-troubleshooting.md
- docs/prompts/20251221-memories_populate-implementation.md
- docs/prompts/20251226-remove-legacymind_update-memories_populate.md
- docs/prompts/20251227-crates-updates.md
- docs/prompts/20251227-technical-debt-cleaning.md
- docs/prompts/20251227-technical-debt-cleaning0.md
- docs/prompts/20251227-technical-debt-cleaning1.md
- docs/prompts/20251227-technical-debt-cleaning2.md
- docs/prompts/20251227-technical-debt-cleaning3.md
- docs/prompts/20251227-technical-debt-cleaning4.md
- docs/prompts/20251227-technical-debt-cleaning-phase2.md

## Code sections to remove or modify
- src/tools/inner_voice.rs: remove the entire module (params/runtime/context structs, run_inner_voice, handle_inner_voice_retrieve, planner parsing, synthesis, auto-extract to KG, DB writes with origin 'inner_voice', env var warnings).
- src/tools/inner_voice/providers.rs: remove provider helpers (grok_call, allow_grok, fallback_from_snippets, parse_planner_json, compute_auto_extract, etc).
- src/tools/mod.rs: drop `pub mod inner_voice;`.
- src/server/router.rs: remove inner_voice schema wiring, Tool entry in list_tools, and the call_tool match arm that dispatches to handle_inner_voice_retrieve.
- src/schemas.rs: remove Snippet/Diagnostics/RetrieveOut structs if only used by inner_voice; remove inner_voice_schema() and inner_voice_output_schema(); remove inner_voice from detailed_help_schema enum.
- src/tools/detailed_help.rs: remove inner_voice from roster output and the inner_voice help branch.
- src/tools/maintenance.rs: remove inner_voice subsection from echo_config output.
- src/config.rs: remove InnerVoiceConfig, load_from_env, validate, runtime.inner_voice field, and all SURR_INNER_VOICE*/SURR_IV*/IV_* env var handling.
- src/main.rs: update the "Loaded X MCP tools" log line to reflect the new roster.
- tests/inner_voice_*.rs: delete these tests entirely.
- tests/tool_schemas.rs: remove inner_voice from expected tool list and detailed_help schema enum expectations.
- scripts/iv_extract.js, lib/iv_utils.js: delete if unused after tool removal; ensure no other scripts depend on them.
- scripts/package.json: remove inner_voice description or delete script package if only used for iv_extract.
- README.md, GEMINI.md, docs/AGENTS/*, docs/prompts/*, docs/troubleshooting/*, CHANGELOG.md: remove or update all inner_voice references to avoid stale docs.

## Step-by-step removal sequence
1. Delete inner_voice implementation files: `src/tools/inner_voice.rs` and `src/tools/inner_voice/providers.rs`.
2. Remove the module export in `src/tools/mod.rs`.
3. Remove tool registration and routing in `src/server/router.rs` (schemas + Tool list entry + call_tool match arm).
4. Remove schemas and output structs in `src/schemas.rs`, and update the detailed_help schema enum to drop inner_voice.
5. Remove inner_voice runtime config in `src/config.rs` (struct, defaults, env parsing, validation, runtime field).
6. Remove inner_voice references from `src/tools/detailed_help.rs` and `src/tools/maintenance.rs`.
7. Update `src/main.rs` tool roster log message.
8. Delete inner_voice-specific tests and update `tests/tool_schemas.rs`.
9. Remove iv_extract scripts/libs and clean `scripts/package.json` (or delete the scripts folder if it only serves inner_voice).
10. Update docs and prompts to eliminate inner_voice references; add a CHANGELOG entry documenting the removal.
11. Final sweep: ensure no `inner_voice`/`Inner Voice`/`IV_*` references remain.

## Testing / verification after removal
- `rg -n "inner_voice|Inner Voice|IV_|SURR_IV_|SURR_INNER_VOICE|IV_ALLOW_GROK|INNER_VOICE" .` returns no hits in repo.
- `cargo test` (or at least `cargo test --tests`) passes after deleting inner_voice tests and updating tool schemas.
- `cargo clippy` or `cargo check` succeeds with no unused code or unused dependency warnings.
- Run the server (stdio or http) and verify:
  - list_tools no longer returns inner_voice.
  - detailed_help roster omits inner_voice and still validates schema.
  - maintenance_ops:echo_config still returns expected runtime config without inner_voice block.
- If you keep any scripts folder, run its existing tests (if any) to ensure nothing still imports iv_utils.

## Dependencies / side effects to consider
- Removal eliminates retrieval + synthesis + auto-extract workflow; update any client workflows that relied on inner_voice (e.g., prompts that call it) to use legacymind_search + delegate_gemini or other replacements.
- Environment variables become obsolete: SURR_ENABLE_INNER_VOICE, SURR_DISABLE_INNER_VOICE, SURR_INNER_VOICE_*, SURR_IV_*, IV_ALLOW_GROK, INNER_VOICE_LOCAL_FALLBACK, and any IV_* CLI vars referenced by scripts. Remove these from launchd/env configs to avoid confusion.
- Grok-specific config (GROK_BASE_URL/GROK_MODEL) may become unused; confirm whether any other tool relies on Grok before removing.
- Data already written with origin 'inner_voice' (e.g., kg_*_candidates) will remain; decide whether to leave as historical data or add a cleanup step.
- Cargo dependencies used only by inner_voice (reqwest, regex, unicode-normalization, once_cell, blake3, etc.) may become unused; remove them from Cargo.toml once confirmed by `cargo check`/`cargo machete`.
- Ensure docs/prompts that describe inner_voice as a core workflow are revised to prevent misleading guidance.

## Sonnet Verification Notes

**Date**: 2025-12-30
**Verifier**: CC (Sonnet 4.5)
**Status**: Plan verified with corrections and warnings below

### ✅ Verified as Accurate

1. **File inventory**: All runtime/source files correctly identified
   - `src/tools/inner_voice.rs` (1585 lines)
   - `src/tools/inner_voice/providers.rs` (88 lines)
   - All module/router/schema references present as documented

2. **Test files**: All 4 inner_voice test files confirmed
   - `tests/inner_voice_retrieve.rs` (10,366 bytes)
   - `tests/inner_voice_edge_cases.rs` (850 bytes)
   - `tests/inner_voice_providers_gate.rs` (1,760 bytes)
   - `tests/inner_voice_flow.rs` (EMPTY - only 1 byte, can be deleted immediately)

3. **Scripts**: JavaScript extraction tools verified
   - `scripts/iv_extract.js` exists
   - `lib/iv_utils.js` exists
   - `scripts/package.json` correctly describes "inner_voice extraction"
   - No other scripts reference these (grep confirmed)

4. **Documentation references**: All 19 markdown files identified correctly

5. **Environment variables**: Complete list verified via grep in `src/config.rs` and `src/tools/inner_voice.rs`

6. **Removal sequence**: Order is dependency-safe
   - Starting with implementation files prevents lingering references
   - Config removal after tool removal is correct
   - Docs last is appropriate

### ⚠️ Critical Warnings & Missing Items

1. **Snippet struct is SHARED with curiosity tool**
   - **WRONG in plan**: "remove Snippet/Diagnostics/RetrieveOut structs if only used by inner_voice"
   - **REALITY**: `src/tools/curiosity.rs` uses `Snippet` struct (line 192-217)
   - **ACTION REQUIRED**: Only remove `Diagnostics` and `RetrieveOut`. Keep `Snippet` or refactor curiosity first
   - **Location**: `src/schemas.rs` lines 122-136 (Snippet), 138-149 (Diagnostics), 151-161 (RetrieveOut)

2. **Dependency cleanup is incomplete**
   - `blake3`: ONLY used by inner_voice (safe to remove)
   - `unicode-normalization`: ONLY used by inner_voice (safe to remove)
   - `regex`: Used by `src/bin/kg_dedupe_plan.rs` and `src/clients/gemini.rs` (KEEP)
   - `once_cell`: Used by `src/clients/gemini.rs` and `src/cognitive/mod.rs` (KEEP)
   - `reqwest`: Used extensively in `src/embeddings.rs`, `src/utils/db.rs`, `src/clients/gemini.rs` (KEEP)

3. **KG candidate tables remain in schema**
   - `kg_entity_candidates` and `kg_edge_candidates` are defined in `src/server/schema.rs` (lines 120-128)
   - `src/indexes.rs` defines indexes for these tables (lines 98, 107)
   - Plan says "leave as historical data" but doesn't mention removing schema/index definitions
   - **DECISION NEEDED**: Remove table/index definitions or keep infrastructure for future use?

4. **Test file count mismatch**
   - Plan lists 4 test files (correct)
   - But `tests/tool_schemas.rs` expects 6 tools (line 29), needs update to 9 tools (current actual count)
   - After removing inner_voice: should be 9 tools listed in main.rs but test expects 6
   - **MISSING**: Update test_list_tools_returns_expected_tools to expect 9 tools and update the array

5. **Main.rs tool count is already wrong**
   - `src/main.rs` line 89 says "Loaded 10 MCP tools" but includes inner_voice
   - After removal: should say "Loaded 9 MCP tools"
   - Current list in main.rs: legacymind_think, maintenance_ops, memories_create, detailed_help, inner_voice, legacymind_search, delegate_gemini, curiosity_add, curiosity_get, curiosity_search
   - **Corrected list after removal**: legacymind_think, maintenance_ops, memories_create, detailed_help, legacymind_search, delegate_gemini, curiosity_add, curiosity_get, curiosity_search

### 📝 Suggested Refinements to Plan

**Step 4 correction** (src/schemas.rs):
```
Remove ONLY Diagnostics and RetrieveOut structs.
Keep Snippet struct (used by curiosity_search).
Update detailed_help schema enum to drop inner_voice.
```

**Step 6 addition** (Cargo.toml):
```
After step 6, add:
6a. Remove ONLY these Cargo dependencies:
    - blake3 = "1.5"
    - unicode-normalization = "0.1"
Keep: reqwest, regex, once_cell (used by other tools)
```

**Step 8 addition** (tests/tool_schemas.rs):
```
Update test_list_tools_returns_expected_tools:
- Change expected_tools array to 9 tools (remove "inner_voice")
- Update assertion from "6 entries" to "9 entries"
- Update detailed_help enum expectation to match (remove inner_voice from line 81)
```

**Step 11 additions**:
```
OPTIONAL (requires decision):
- Remove kg_entity_candidates and kg_edge_candidates table definitions from src/server/schema.rs
- Remove corresponding index definitions from src/indexes.rs
- Or document that these tables remain for future KG extraction features
```

### 🔍 Additional Edge Cases Discovered

1. **CHANGELOG entry exists** (line mentioning inner_voice in Dec 24 entry)
   - Needs update in step 10 to note this is now legacy

2. **Empty test file**: `tests/inner_voice_flow.rs` is literally empty (1 byte)
   - Can be deleted immediately, won't break anything

3. **No integration test coverage**: Tests import inner_voice functions directly
   - After removal, no test validates that inner_voice is truly gone from MCP interface
   - Consider: add test to mcp_protocol.rs that verifies list_tools doesn't include inner_voice

4. **SURR_IV_TEST_CANDIDATES**: Special test env var used in inner_voice.rs
   - Found at lines ~942 and ~1420 in inner_voice.rs
   - Not mentioned in env var list but should be added to removal notes

### 🎯 Verification Summary

**Plan completeness**: 85% accurate
**Critical blockers**: 1 (Snippet struct sharing)
**Recommended additions**: 4 (dependency cleanup specifics, test updates, schema decision, CHANGELOG)
**Risk level**: Low (if Snippet struct handling is corrected)

The removal sequence is sound, but executor must:
1. NOT remove Snippet struct (curiosity dependency)
2. Only remove blake3 + unicode-normalization deps
3. Update test expectations for 9 tools (not 6)
4. Decide on KG candidate table schema fate

---

## Gemini Assessment

**Date**: 2025-12-30
**Assessor**: Gemini 2.5 Flash
**Focus**: Plan soundness, dead code risk, architecture concerns

### Overall Verdict
With Sonnet's corrections, the plan is **sound and low-risk** IF clarifications are made around three key areas: Snippet struct handling, dependency cleanup rigor, and KG schema fate.

### Sequence Safety ✅
The removal order is logically correct:
- Implementation files first → prevents lingering references
- Router/schema next → breaks external access
- Config removal → stops initialization
- Tests and docs last → cleanup after code is gone
- Dead code sweep → catches anything missed

Sequence will not leave the system in a broken intermediate state if followed cleanly.

### Dead Code Risk Assessment ⚠️

**HIGH RISK AREA - KG Schema:**
The plan identifies `kg_entity_candidates` and `kg_edge_candidates` tables but doesn't decisively address them. Leaving unused schema tables is the primary vector for dead code. **Critical decision needed:**
- If these tables are only used by inner_voice's auto-extract feature → DELETE schema definitions from `src/server/schema.rs` and indexes from `src/indexes.rs`
- If kept for future use → document this explicitly in CHANGELOG; don't leave orphaned

**MEDIUM RISK - Snippet Struct:**
Sonnet's finding is correct: `curiosity_search` uses Snippet. The plan must be clear:
- Keep struct definition
- Remove only inner_voice-specific Snippet handling logic
- Remove inner_voice-specific test cases that exercise Snippet
- This is a refactor, not a simple deletion

**LOW RISK - Dependencies:**
Gemini confirms Sonnet's assessment: `blake3` and `unicode-normalization` are safe removals. All others (`reqwest`, `regex`, `once_cell`) are verified in-use elsewhere.

**DEAD CODE SWEEP RIGOR:**
The "final sweep" needs teeth. Specifically:
```bash
rg -n "inner_voice|Inner Voice|IV_|SURR_IV_|SURR_INNER_VOICE|IV_ALLOW_GROK|INNER_VOICE|Snippet.*inner_voice" .
```
Should return ZERO hits. If Snippet is mentioned alongside inner_voice in any comment/doc, it needs review.

### Architecture Concerns 🏛️

**Loss of Synthesis Workflow:**
inner_voice provided retrieval + synthesis + auto-extract. The plan assumes this functionality is no longer needed or has been replaced. Gemini assessment: **verify clients aren't still calling it.** If any prompt docs or delegate_gemini chains expect inner_voice, those break.

**KG Data Consistency:**
Data already written with origin='inner_voice' to kg_* tables will become orphaned if the tables are deleted. **Decision required:**
- Migrate/archive existing rows
- Leave as historical (mark as deprecated)
- Delete outright (data loss, acceptable if experimental feature)

This should be documented in CHANGELOG so future developers understand the data gap.

**Schema Deprecation Pattern:**
If KG tables are kept for "future use," establish clear deprecation language:
```rust
// DEPRECATED: kg_*_candidates tables remain from removed inner_voice tool
// These tables are not currently used. Remove in next major version if no adoption.
```
This prevents accidental reuse of orphaned schema.

### Specific Recommendations from Gemini

1. **KG Schema - Make a Decision:**
   - Add explicit step: "Analyze data in kg_entity_candidates and kg_edge_candidates; decide: migrate to archive table, delete entirely, or keep as deprecated."
   - Document decision in CHANGELOG
   - If deleted: remove schema definitions AND update Sonnet's suggested index cleanup

2. **Snippet Struct - Add Clarification:**
   - Plan should state: "Only remove inner_voice-specific logic using Snippet. Struct remains for curiosity_search."
   - Consider: does Snippet need restructuring if inner_voice's logic was core to its design?

3. **Dependency Cleanup - Verify Rigorously:**
   - After removing inner_voice code, run `cargo machete --fix` to auto-detect unused deps
   - Only remove what `cargo machete` identifies as unused (should be exactly blake3 + unicode-normalization)

4. **Test Verification - Add to Final Sweep:**
   - `cargo test` must pass
   - `cargo clippy` must show zero warnings
   - `rg` dead code search must return zero hits
   - Server startup must succeed and list exactly 9 tools (not 10)

### Risk Summary

| Risk | Level | Mitigation |
|------|-------|-----------|
| Snippet struct removal | Medium | Clarify: only remove inner_voice logic, not struct |
| KG schema orphaning | Medium | Make explicit decision on schema fate |
| Dependency cleanup | Low | Use cargo machete to verify |
| Intermediate broken state | Low | Sequence order is correct |
| Dead code remaining | Low | Final rg sweep catches orphans |

### Conclusion

The removal plan is **ready to execute** with these clarifications:
1. Explicitly address KG schema fate (DELETE vs DEPRECATE)
2. Confirm Snippet struct is retained (only inner_voice logic removed)
3. Run `cargo machete` as final verification
4. Document data implications in CHANGELOG

Once these decisions are made and documented, the plan carries low execution risk and will cleanly remove inner_voice from the codebase.
