---
id: doc-4
title: Implementation Steps - remove scalpel tool and local delegation
type: other
created_date: '2026-01-09 15:10'
updated_date: '2026-01-09 15:10'
---
# Implementation Steps - remove scalpel tool and local delegation

Linked task: `backlog/tasks/task-40 - Remove-scalpel-tool-and-local-delegation.md`

## Outcome
Fully excise the scalpel tool (code, tests, docs, configs, backlog) while keeping the project compilable at every step, freeing port 8111, and ensuring clients gracefully handle the missing tool. Preserve history via archive; avoid serde panics on existing DB rows.

## Scope (remove)
- Implementation + helpers: `src/tools/scalpel.rs`, `src/server/scalpel_helpers.rs`, `src/clients/local.rs`.
- Tests/scripts: `tests/test_scalpel_operations.rs`, `tests/test_append_behavior.rs` (scalpel-only), `scripts/start_scalpel_server.sh`.
- Registry/plumbing: `src/registry.rs`, `src/schemas.rs`, `src/tools/mod.rs`, `src/clients/mod.rs`, `src/server/mod.rs`, `src/server/router.rs`.
- Config/docs: `.env.example` scalpel vars, CHANGELOG/README, `docs/AGENTS/*.md` scalpel sections, system prompts mentioning scalpel.
- Backlog/docs: task-30/32/34/37/38 (completed), task-35/36 (blocked), task-39 (obsolete), percent-encoded duplicate `backlog/tasks/task-35%20-%20Scalpel-KG-prompt-tuning.md`.

## Pre-flight
- Create safety branch: `git checkout -b backup/remove-scalpel`.
- Snapshot current env flags/secrets for reference; note any launchd bindings to 8111.
- Confirm no active workflows rely on scalpel (CI, scripts, MCP clients).

## Step-by-step implementation
1) **Quiesce + registry first (keeps builds green)**
   - Remove scalpel from `src/registry.rs`, `src/schemas.rs`, `src/server/router.rs`, `src/server/mod.rs`, `src/tools/mod.rs`, `src/clients/mod.rs`.
   - If enums previously had a `Scalpel` variant, add `#[serde(other)] DeprecatedTool` or temporarily alias to avoid deserialization panic until DB cleanup runs.
2) **Delete implementation surface**
   - Remove files: `src/tools/scalpel.rs`, `src/server/scalpel_helpers.rs`, `src/clients/local.rs`.
   - Drop scripts/tests: `scripts/start_scalpel_server.sh`, `tests/test_scalpel_operations.rs`, `tests/test_append_behavior.rs`.
3) **Dependency + config pruning**
   - In `Cargo.toml`, drop scalpel-only deps/features (e.g., mistralrs/middleware, reqwest configs). Run `cargo tree -i mistralrs-server` to verify zero remaining references.
   - Strip scalpel env vars from `.env.example` and any config defaults; remove port 8111 mentions.
4) **Backlog + docs**
   - Move listed tasks to `backlog/archive/scalpel/`; mark task-39 obsolete; delete percent-encoded duplicate.
   - Update README, CHANGELOG, `docs/AGENTS/tools.md`, `docs/AGENTS/maintenance.md`, any API/OpenAPI/exported schema describing tool lists.
   - Adjust system prompts / MCP config that advertise scalpel.
5) **Data hygiene (optional but recommended)**
   - Clear job records: `DELETE agent_jobs WHERE tool_name = 'scalpel';`.
   - Decide on historical thoughts with `origin = 'scalpel'` (keep for history; note in CHANGELOG).
6) **Observability cleanup**
   - Remove or rename metrics/log keys prefixed `scalpel_*`; drop alerts/dashboards expecting port 8111.
7) **Validation**
   - `cargo fmt --all`
   - `cargo check --workspace --all-targets`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace --all-features`
   - `grep -r "scalpel" {src,tests,docs,.env.example}` → expect 0
   - Smoke `call_gem`: queued + sync path, `call_jobs`/`call_cancel` sanity.
8) **Release**
   - Review `git status`/`git diff`.
   - Commit message: `Remove scalpel tool and local delegation infrastructure`.
   - If MCP schemas are generated, regenerate artifacts/build outputs.

## Notes / decisions to lock in
- Prefer deleting stale DB rows over keeping serde fallbacks long-term; keep fallback only if migration can’t be run immediately.
- Keep archival docs in a dedicated `backlog/archive/scalpel/` folder for discoverability.
- Ensure router/list-tools responses shrink by one tool and clients tolerate the delta (no ordinal assumptions).

## Acceptance gates
- Builds/tests/clippy clean; zero `scalpel` grep hits.
- No scalpel tool appears in MCP tool listing; port 8111 unused.
- Cargo tree shows no mistralrs/local delegation deps.
- Backlog/doc set reflects removal; archive created; duplicate task file removed.
- call_gem delegation unaffected (smoke tests pass).

## Gap Analysis (Vibe)

### Items in Task File Not in Implementation Steps

1. **Additional files to remove**:
   - `tests/test_append_behavior.rs` (mentioned in Gemini review but not in original task)
   - `src/clients/local.rs` (mentioned in Gemini review)

2. **More detailed testing strategy**:
   - Task mentions integration tests, performance tests, memory usage tests
   - Task suggests API documentation updates, user guides, troubleshooting guides
   - Task includes monitoring/observability cleanup (metrics, logging, alerts)

3. **Security considerations**:
   - Task mentions port security verification
   - Task suggests dependency removal for attack surface reduction
   - Task includes secret management verification

4. **User communication**:
   - Task suggests deprecation notice period
   - Task mentions migration guide for scalpel users
   - Task includes release notes documentation

5. **CI/CD updates**:
   - Task mentions CI pipeline updates
   - Task suggests build validation steps
   - Task includes test coverage verification

6. **Performance optimization**:
   - Task suggests measuring code size reduction
   - Task mentions build time improvement measurement
   - Task includes runtime performance verification

7. **Future-proofing**:
   - Task suggests modular design review
   - Task mentions documenting lessons learned
   - Task includes architecture review opportunity

### Items in Implementation Steps Not in Task File

1. **More granular implementation sequence**:
   - Implementation steps suggest doing registry/plumbing cleanup first to keep builds green
   - Implementation steps include specific enum handling for serde compatibility
   - Implementation steps mention cargo tree verification for dependency removal

2. **Database hygiene**:
   - Implementation steps include specific SQL for cleaning up agent_jobs
   - Implementation steps mention handling thoughts with scalpel origin

3. **Observability cleanup**:
   - Implementation steps mention removing scalpel-specific metrics/log keys
   - Implementation steps include dropping alerts/dashboards for port 8111

4. **Validation sequence**:
   - Implementation steps have a more detailed validation sequence
   - Implementation steps include specific grep pattern for validation

### Key Differences

1. **Order of operations**: Implementation steps prioritize keeping builds green by doing registry cleanup first, while task file has a more traditional file deletion approach.

2. **Database handling**: Implementation steps are more specific about database cleanup operations and considerations.

3. **Dependency management**: Implementation steps include more detailed dependency removal verification using cargo tree.

4. **Testing scope**: Task file has a broader testing strategy including performance, security, and user communication aspects.

5. **Future considerations**: Task file includes more forward-looking items like architecture review and lessons learned documentation.

### Recommendations

The implementation steps should be enhanced with:
- The broader testing strategy from the task file
- Security considerations and user communication planning
- Performance optimization measurements
- Future-proofing documentation

The task file could benefit from:
- The more granular implementation sequence to keep builds green
- Specific database cleanup operations
- Detailed dependency removal verification

## Questions and Clarifications

After reviewing the task file, implementation steps, and current codebase, I have the following questions:

1. **Task-39 Obsolescence**: The task file lists task-39 as obsolete, but it is currently in `backlog/active/`. Should it be moved to `backlog/archive/scalpel/` or a general archive? Also, confirm if it needs to be marked as cancelled or completed before moving.

2. **Database Cleanup Timing**: The implementation steps recommend data hygiene (clearing job records and deciding on thoughts with scalpel origin), but is this to be done before or after the code removal? If before, to prevent new entries, but since we're removing the tool, perhaps after is fine.

3. **Port 8111 Verification**: Confirm that port 8111 is only used by the scalpel server. Should I check for any launchd or systemd bindings in the system?

4. **CHANGELOG Update Specifics**: For the CHANGELOG, should the scalpel removal be added as a new entry, or update the existing scalpel configuration entry?

5. **No Additional References**: From my search, the primary references are in the files listed. Confirm if there are any in README.md or system prompts beyond what's mentioned.

6. **Dependency Removal Confirmation**: Since Cargo.toml doesn't list mistralrs, and reqwest is general, confirm if there are any features or conditional dependencies to remove.

7. **Archive Creation**: Should the `backlog/archive/scalpel/` folder be created manually, or is it assumed to exist?

Ready to proceed once these are clarified.

## Codex Answers to Grok Questions
1) **Task-39 Obsolescence**: Move `backlog/active/task-39 - Fix-Hermes-3-tool-use-protocol.md` into `backlog/archive/scalpel/` and mark it obsolete/cancelled; it is scoped to scalpel and should live with the other archived scalpel tasks.
2) **Database Cleanup Timing**: Run `DELETE agent_jobs WHERE tool_name = 'scalpel';` right after code/routes/registry removal (while the serde fallback is still present if you add one). That ensures no stale rows linger before you drop any `Scalpel` enum variants/`#[serde(other)]` shim.
3) **Port 8111 Verification**: Scalpel was the only planned consumer. Sanity-check launch agents/scripts for stray bindings (`rg "8111" backlog scripts src`), and if paranoid run `lsof -i :8111` during validation to confirm nothing is listening.
4) **CHANGELOG Update**: Add a new dated entry (or under Unreleased) explicitly noting scalpel removal and the rationale (unreliable on 32GB Studio; remote `call_gem` preferred). Don’t rewrite old entries.
5) **No Additional References**: Expect stragglers only in README/AGENTS docs/system prompts. The existing grep step across `{src,tests,docs,.env.example}` plus prompts should surface them; no other hidden references are known beyond what’s listed.
6) **Dependency Removal Confirmation**: There are no dedicated mistralrs/local-delegation features left once `src/clients/local.rs` and scalpel modules are gone, but still drop any scalpel-specific dependencies/feature flags if present and verify with `cargo tree -i mistralrs-server`/`cargo tree -i local`.
7) **Archive Creation**: Create `backlog/archive/scalpel/` if it doesn’t exist and place the completed/blocked scalpel tasks (including the percent-encoded duplicate and task-39) there for history.
