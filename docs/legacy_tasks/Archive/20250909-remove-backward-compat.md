# Remove Backward Compatibility Paths — Implementation Plan

Owner: Codex • Date: 2025‑09‑09 • Target: master (post‑revert to known‑good)

Purpose
- Remove all backward‑compatibility aliases, legacy tool stubs, and confusing schema enumerations from the Surreal Mind MCP server. CC and Warp won’t reference old tools once brain files are updated, so compatibility shims are unnecessary bloat and a source of bugs.

Non‑Goals
- No behavior change to the active, supported tools.
- No changes to DB schema, embeddings, or retrieval logic.
- Do not remove both transports; keep stdio and HTTP MCP as first‑class (operator chooses per launcher).

Current State (observed)
- Tools registry includes only the unified tools, but code still contains:
  - An alias for `inner_voice.retrieve` → `inner_voice`.
  - Detailed help/tool schemas that enumerate legacy tools (e.g., `think_convo`, `think_plan`, `think_build`, `think_debug`, `think_stuck`) even though they aren’t registered.
  - (After revert) thread helpers (`thoughts.thread`, `thoughts.links`) were deleted at file level in one branch but may still be referenced in server registrations/handlers in another. Ensure they’re fully removed.
  - Test/script artifacts that still refer to old names (e.g., `tests/test_detailed_mcp.sh` uses `think_convo`).

Supported Tools (keep)
- legacymind_think
- inner_voice (no alias)
- memories_create
- memories_moderate
- legacymind_search
- photography_think
- photography_memories
- photography_search
- maintenance_ops
- detailed_help (but update its schema to enumerate only supported tools)
- (Optional) update_docs once merged — leave placeholder wiring off until implemented.

Remove (backward‑compat and dead code)
1) Aliases
   - Remove alias route: `"inner_voice.retrieve"` → handler mapping.

2) Legacy tool names from schemas/help
   - In schemas used by `detailed_help`, drop all legacy enums: `think_convo`, `think_plan`, `think_build`, `think_debug`, `think_stuck`.
   - Ensure no other schema or help text mentions obsolete tools.

3) Thread convenience tools (if present)
   - Remove tool registrations for `thoughts.thread` and `thoughts.links` if handlers/files are gone.
   - If handlers exist, delete them and their export in `src/tools/mod.rs`.

4) Test/scripts referencing legacy tools
   - Update or remove `tests/test_detailed_mcp.sh` lines that call `think_convo` or other legacy names.
   - Update tool schema tests to match the new minimal tool set.

5) Code comments and AGENTS.md
   - Update AGENTS.md and any comments listing tools to reflect the final set only.

Step‑by‑Step Implementation
1) Registry cleanup
   - File: `src/server/mod.rs`
     - list_tools(): remove `inner_voice.retrieve`, `thoughts.thread`, `thoughts.links` entries if present.
     - call_tool(): remove match arms for the same.

2) Handlers removal
   - Files: `src/tools/inner_voice.rs` (only alias route removal; keep primary), `src/tools/thoughts_thread.rs`, `src/tools/thoughts_links.rs` (delete if present), `src/tools/mod.rs` (drop `pub mod` exports).

3) Schema/help tightening
   - File: `src/schemas.rs` (or wherever tool schemas are defined)
     - In `detailed_help_schema()`, shrink the `tool` enum to only the supported tool names (see “Supported Tools”).
     - Ensure `legacymind_think_schema()` has no legacy hints that imply separate tools.

4) Tests and scripts
   - File: `tests/test_detailed_mcp.sh`: replace `think_convo` with `legacymind_think` and adapt its minimal arguments (`{"content":"…"}`).
   - File: `tests/tool_schemas.rs` (if any legacy assertions) — update to assert only the supported tool names.

5) Ripgrep sweep (acceptance gate)
   - rg for: `inner_voice.retrieve`, `think_convo`, `think_plan`, `think_build`, `think_debug`, `think_stuck`, `thoughts.thread`, `thoughts.links`.
   - Expect 0 hits in src/ (except in CHANGELOG or historical docs).

6) Docs
   - File: `AGENTS.md` — update “Exposed MCP Tools” to list only supported tools.

7) Build & smoke
   - `cargo build --release` must pass.
   - Stdio handshake: initialize → tools/list, confirm only supported names.
   - HTTP handshake (if enabled): POST initialize + tools/list, same result.

Acceptance Criteria
- tools/list returns only: legacymind_think, inner_voice, memories_create, memories_moderate, legacymind_search, photography_think, photography_memories, photography_search, maintenance_ops, detailed_help. (Plus update_docs if/when landed.)
- No alias for `inner_voice.retrieve`; stdio/HTTP both work.
- rg across repo shows 0 code refs to legacy names listed above.
- Tests updated; `tests/test_detailed_mcp.sh` passes with legacymind_think.

Rollback
- All changes are deletions/registry edits. If needed, restore via git revert of the PR.

Risks
- Launchers that cached old tool names will error until brain files are updated. Mitigation: We’re explicitly updating CC/Warp brain files as part of rollout.
- If thread tools were still used by any scripts, removal breaks them. Mitigation: they were already deleted in prior branch; confirm no callers remain (rg + CI).

Execution Notes
- Keep both transports supported (stdio + HTTP). This plan only removes tool‑level BC, not transport.
- Keep MCP_NO_LOG default behavior (stdio log silence) to avoid stdio corruption.

Timeline
- Code changes + tests: ~45–60 minutes.
- Smoke (stdio + HTTP): ~10 minutes.
- Documentation touch‑ups: ~10 minutes.

## Questions

- Are there any external dependencies or integrations that might still rely on the legacy tool names or aliases, beyond what's mentioned in the plan? No
- How exactly will the brain files for CC and Warp be updated to reference the new tool names, and is there a timeline for that to ensure no disruptions? They have already been updated
 - In the step-by-step implementation, when removing thread convenience tools, should we confirm if any handlers or references exist in branches other than the current one? Limit to current master; add a post-merge CI ripgrep check so future branches surface conflicts early.
 - For the ripgrep sweep, are there specific patterns or files to exclude (e.g., historical docs or changelogs) to avoid false positives? Yes: exclude CHANGELOG.md, AGENTS.md, fixes/** and docs/**; scan only src/** and tests/**.
- Will there be any impact on existing user configurations or saved sessions that might reference the old tool names? No
- How do we plan to handle potential errors in launchers that haven't updated their brain files yet, and is there a graceful degradation mechanism? There are none.
 - Are there any automated tests or scripts that need to be updated beyond the ones listed, such as integration tests or CI pipelines? Yes: update tests/mcp_integration.rs and tests/tool_schemas.rs if they enumerate legacy names; also adjust tests/relationship_smoke.rs only if it imports removed helpers.
- In terms of rollback, is there a way to temporarily re-enable the legacy tools if needed during the transition period? This is not necessary.
