# SurrealMind rmcp 3.1.4 — Implementation Plan

**Status:** Source candidate complete for Phases 1-5 and 7 (compiled, lint-clean, `--release` builds); Phase 6 has a known HTTP-test gap (no HTTP-01..09 runtime coverage); Phase 8's CC-review gate is NOT closed (R4 reconciliation and R5 deployed-evidence both open, review not yet requested). **This is not yet "Implementation Complete" by this document's own Phase 8 gate.** Production install/restart NOT performed.  
**Parent:** [`rmcp-3.1.4-upgrade.md`](rmcp-3.1.4-upgrade.md)  
**Depends On:** CC plan approval  
**Testing:** [`rmcp-3.1.4-upgrade-testing.md`](rmcp-3.1.4-upgrade-testing.md)

## Goal

Produce a reviewable, warning-clean rmcp 3.1.4 candidate without changing SurrealMind's intended tool behavior or touching the live process before the deployment gate.

## Phase 0 — Establish the isolated baseline

- [ ] Re-read Studio `HEAD`, branch, and `git status`; record any drift from `551c1f8`.
- [ ] Identify whether any pre-existing dirty file overlaps the planned source paths.
- [ ] Create a dedicated branch and isolated git worktree from the agreed source commit.
- [ ] Record the live binary SHA-256, launchd label, running PID, `/health`, public endpoint status, and exact `tools/list` result.
- [ ] Copy the live binary to a dated rollback directory and verify its hash and executable mode.
- [x] Capture an out-of-band Cloudflare-origin `Host` witness without touching SurrealMind: CC measured the existing `lightroom-demo` ingress forwarding its original public hostname.
- [ ] Record the residual inference from the measured `127.0.0.1:8080` ingress to SurrealMind's `localhost:8787` ingress; keep direct public-candidate acceptance load-bearing.

**Gate:** Source identity, dirty-state boundary, rollback artifact, current public baseline, and observed Host value are all recorded before source edits.

## Phase 1 — Add explicit runtime configuration

- [x] Add `RuntimeConfig::http_allowed_hosts: Vec<String>`.
- [x] Parse `SURR_HTTP_ALLOWED_HOSTS` as a comma-separated, trimmed, non-empty list of additional hosts.
- [x] Always seed the allowlist with `localhost`, `127.0.0.1`, and `::1`; a valid configured list extends and deduplicates that baseline rather than replacing it.
- [x] Fail startup loudly when the variable is present but empty, contains an empty entry, or otherwise cannot yield a valid allowlist.
- [x] Add unit tests for absent/default, configured-extension, loopback retention, whitespace, duplicate, empty, and malformed-entry behavior. (10 tests in `src/config.rs`, all passing under `cargo test --lib`.)
- [x] Add the measured public-origin host to deployment configuration without embedding a machine-only magic constant in Rust. (Not embedded — `default_http_allowed_hosts()` only seeds the loopback set; `mcp.samataganaphotography.com` is documented as the required deployment value, not hardcoded.)
- [x] Document the setting in README/AGENTS connection or maintenance documentation. (`README.md`, `docs/AGENTS/connections.md`.)

**Gate:** The production Host value is declared and falsifiable; one production configuration accepts both loopback and the public hostname; no code path silently removes loopback or turns an empty value into allow-all.

## Phase 2 — Dependency bump and compiler inventory

- [x] Change only `rmcp` from `0.16.0` to exact `3.1.4`; keep all four feature flags. Do not change the `version` field in `Cargo.toml` as a side effect — record that decision explicitly rather than letting it drift (see upgrade doc Constraint 9). (`Cargo.toml`'s `[package].version` remains `0.8.2`; the CHANGELOG's Unreleased entry records this as a deliberate decision, not a silent omission.)
- [x] Run `cargo check --message-format=short` and save the complete error inventory in the testing document. This first run is expected to move the lockfile; treat that as the one authorized mutation. (41 errors, 16 deprecation warnings — see testing doc's Compiler Inventory section.)
- [x] From this point forward, every subsequent compiler/build/lint command runs with `--locked` (or, where a flag is unavailable, is immediately followed by `git diff Cargo.lock` to confirm zero unexpected drift). The lockfile is established once this step completes; nothing after it should move it again. (Verified stable — identical `70 lines / 44 insertions / 26 deletions` diff — after every subsequent `cargo check`, `cargo test --no-run`, `cargo clippy`, and `cargo build --release`, all run `--locked`.)
- [x] Run `cargo check --all-targets --features db_integration --message-format=short` after the library reaches a compilable state, so binary and feature-gated failures cannot hide. (Clean; also ran `cargo test --workspace --features db_integration --no-run --locked` clean.)
- [x] Record lockfile changes and verify they are limited to the expected rmcp/rmcp-macros/sse-stream and transitive additions or upgrades; use `cargo tree --locked` for post-update dependency evidence. (See testing doc CMP-01 row and the CHANGELOG's dependency-delta note: `rmcp`/`rmcp-macros` → `3.1.4`, `sse-stream` `0.2.1`→`0.2.5`, `darling`/`darling_core`/`darling_macro` `0.23.0`→`0.24.1`, new `syn 3.0.4` alongside existing `2.0.114`, new direct `indexmap` dependency of rmcp, new `base64 0.23.1` alongside existing `0.22.1`. No unrelated dependency moved.)

**Gate:** Compiler evidence supersedes the research counts; unexplained dependency drift blocks the next phase.

## Phase 3 — Migrate model construction

- [x] Replace all 16 `Tool` literals with `Tool::new(...).with_title(...)` while preserving names, descriptions, input schemas, and ordering. (`src/server/router.rs`.)
- [x] Replace all `CallToolRequestParams` literals with `CallToolRequestParams::new(...).with_arguments(...)`; do not invent `input_responses` or `request_state`. (`src/tools/maintenance.rs`, `src/bin/kg_wander.rs`, and every test-suite call site — `tests/relationship_smoke.rs`, `tests/test_agent_job_status.rs`, `tests/mcp_integration.rs`, `tests/test_wander.rs`, `tests/mcp_protocol.rs`.)
- [x] Rebuild `get_info` with supported constructors/builders. (`ServerCapabilities::builder().enable_tools_with(...)`, `Implementation::new(...).with_title/with_description/with_website_url(...)`, `ServerInfo::new(capabilities).with_server_info(...)`.)
- [x] Preserve `tools.listChanged = false` explicitly. (`ToolsCapability::default()` then field-mutated, per D4.)
- [x] Keep `ListToolsResult` and `ErrorData` construction unchanged where the compiler confirms they remain exhaustive. (Both untouched — compiler raised no non-exhaustive error against either.)
- [x] Add or update exact assertions for all 16 tools, their order, titles, descriptions, and input schemas. (`tests/mcp_protocol.rs::test_list_tools_protocol`, driven through the real protocol harness against a live server — compiled and lint-clean under `--features db_integration`, **not executed**: no verified disposable database was available in this session. Execution is a Testing-phase gate.)

**Gate:** Library compiler errors from non-exhaustive construction and removed `execution`/`task` fields are zero; tool-surface comparison is exact.

## Phase 4 — Adapt response and content types

- [x] Change `ServerHandler::call_tool` to return `CallToolResponse`. (`src/server/router.rs`.)
- [x] Convert `CallToolResult` once after router dispatch; leave individual handler return types unchanged. (`.map(Into::into)` on the match result; all 15 handler modules untouched — D2.)
- [x] Replace `kg_wander`'s `RawContent::Text`/`.raw` read with `ContentBlock::Text` and the typed text field. (`src/bin/kg_wander.rs`; the equivalent pattern in `tests/test_wander.rs` and `tests/mcp_integration.rs` fixed the same way.)
- [x] Add a focused test proving structured tool results still serialize and a focused test proving `kg_wander` extracts text correctly. (Structured-result serialization: existing `tests/relationship_smoke.rs` / `tests/tool_schemas.rs` already exercise `CallToolResult::structured` paths and pass under default features. Content extraction: `tests/test_wander.rs` (fixed `ContentBlock::Text` match, DB-gated, compiled/lint-clean, not executed this session) and `tests/mcp_integration.rs` (same).)

**Gate:** Router, maintenance path, and every build target compile without widening changes into the 15 handler modules.

## Phase 5 — Correct protocol initialization

- [x] Remove the custom `initialize` echo override after confirming it has no hidden project-specific side effect. (`src/server/router.rs` — the override only did `info.protocol_version = request.protocol_version.clone()`, confirmed by reading it before removal; no other side effect existed.)
- [x] Let rmcp negotiate against `supported_protocol_versions`. (Default `supported_protocol_versions()` left unmodified — `ProtocolVersion::KNOWN_VERSIONS`.)
- [x] Narrow `supported_protocol_versions` only if protocol tests prove SurrealMind cannot honor one of rmcp's defaults. (Not narrowed — no test found a reason to.)
- [x] Capture initialize responses for a supported legacy version, the current latest supported version, and an unsupported future version. (`tests/mcp_protocol.rs::test_initialize_protocol_negotiation` — PROTO-01/02/03, plus PROTO-05 capability-serialization assertions on all three. Compiled and lint-clean under `--features db_integration`, **not executed**: no verified disposable database was available in this session.)

**Gate:** Unsupported versions are rejected or negotiated according to rmcp's contract; no arbitrary version is echoed as accepted.

## Phase 6 — Port Streamable HTTP deliberately

- [x] Construct `StreamableHttpServerConfig` from `Default` plus explicit methods for SSE keepalive, legacy-session mode, and allowed hosts. (`src/http.rs` — `StreamableHttpServerConfig::default().with_legacy_session_mode(true).with_sse_keep_alive(...).with_json_response(false).with_allowed_hosts(server.config.runtime.http_allowed_hosts.clone()).with_stateless_protocol_metadata_required(false)`.)
- [x] Preserve JSON-response, session-store, and stateless-metadata behavior as decided in the parent document. (`json_response: false`, `session_store` left absent/`None` — never set — and `stateless_protocol_metadata_required: false`, all explicit per D6.)
- [x] Leave Origin validation disabled only under parent decision D7 and create the separately assigned security follow-up before this parent closes. (`allowed_origins` untouched at rmcp's empty/disabled default; D7's follow-up is recorded in the CHANGELOG's Deferred section and the parent upgrade doc — it is a separate task, not opened here.)
- [x] Verify the 4 MiB default request cap against observed payload sizes and a synthetic boundary test. (Confirmed via source read: `DEFAULT_MAX_REQUEST_BODY_BYTES = 4 * 1024 * 1024` in rmcp 3.1.4's `transport/streamable_http_server/tower.rs`, unchanged by this branch — no `with_max_request_body_bytes` override added, so the cap stays at rmcp's default 4 MiB, resolving the briefing's 4 MiB/10 MB conflict per the parent doc.) **Gap: no synthetic HTTP-07/HTTP-08 boundary test was written or run** — see Known gaps below.
- [x] Confirm `StreamableHttpService` mounts into axum 0.7 after earlier library errors no longer mask the binary build. (Confirmed: `cargo check`/`cargo build --release` both succeed with `axum = "0.7"` unchanged in `Cargo.toml`; `StreamableHttpService::new(...)` mounts via `.nest_service(path.as_str(), mcp_service)` in `src/http.rs` exactly as before, and the release binary link succeeds.)
- [ ] Add focused Host allow/reject tests using the exact production hostname and an unlisted control hostname. **NOT DONE — known gap, see below.**

**Gate:** Local loopback, measured public Host, rejection control, body-size boundary, and legacy-session tests pass. **Not fully met — see Known gaps.**

**Known gaps (Phase 6, honestly unclosed):** `SURR_HTTP_ALLOWED_HOSTS`'s *parsing/extension semantics* are unit-tested exhaustively (Phase 1), but nothing in this pass exercises rmcp's actual `Host`-header accept/reject behavior over a running `StreamableHttpService` (HTTP-01/02/03/04/05), nor the 4 MiB body-size boundary (HTTP-07/HTTP-08) against a live listener. `start_http_server` builds and binds the full axum app in one function with no seam to construct just the `Router`/`Service` for an in-process `tower::Service::oneshot` test without either binding a real port or refactoring `src/http.rs` — judged out of scope for this pass per the instruction to preserve current HTTP semantics with minimal diff, and because these are exactly the "Isolated runtime tests" (RUN-01 preamble, HTTP-01..09) the separate testing document already assigns to a live candidate process. This is a genuine, load-bearing gap for HTTP acceptance, not a paperwork gap: the testing doc's HTTP-01..09 rows must still be executed against a running candidate before deployment.

## Phase 7 — Preserve notification compatibility without normalizing deprecation

- [x] Port `LoggingMessageNotificationParam` through its constructor and logger builder. (`LoggingMessageNotificationParam::new(level, json!(...)).with_logger("surreal-mind")` in `src/tools/test_notification.rs`.)
- [x] Scope `#[allow(deprecated)]` to the smallest function or module that must retain the public tool. (Two narrow scopes only: the `use` import of `LoggingLevel`/`LoggingMessageNotificationParam`, and the `handle_test_notification` function body. No crate-wide or module-wide allowance.)
- [x] Add a rationale naming SEP-2577 and the separate retirement/replacement task. (Doc comments on both `#[allow(deprecated)]` sites in `src/tools/test_notification.rs`.)
- [x] Verify `test_notification` remains listed and still emits a client-observable notification. (`tests/mcp_protocol.rs::test_notification_protocol_bridge` — TOOL-06, drives a real `test_notification` call through the protocol harness and asserts both the response and the notification arrive. Compiled and lint-clean under `--features db_integration`, **not executed**: no verified disposable database was available in this session.)

**Gate:** The public tool remains functional and the whole workspace is warning-clean under `-D warnings`. (`cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` exits 0, zero warnings — verified after every phase's edits, not just once at the end.)

## Phase 8 — Documentation and implementation review

- [x] Update `CHANGELOG.md` with the dependency, compatibility decisions, Host allowlist requirement, protocol negotiation correction, and rollback note. (New `[Unreleased] - rmcp 3.1.4 upgrade` section. No rollback note beyond "not installed" — the actual rollback artifact/procedure is Phase 0/Phase 9 deployment-gate work this pass did not perform; see Known gaps.)
- [x] Update README/AGENTS material for new environment configuration and acceptance commands. (`README.md` — `SURR_HTTP_ALLOWED_HOSTS` documented, missing `test_notification` tool-table row added, stray formatting fixed. `docs/AGENTS.md` — roster line corrected to 16 tools including `journal`/`test_notification`. `docs/AGENTS/connections.md` — Host allowlist documented against the Cloudflare tunnel entry. `docs/AGENTS/tools.md` — missing `journal`/`test_notification` rows added, stray formatting fixed.)
- [x] Correct the stale startup log that says 15 tools and omits `journal` ... **Also check `src/tools/howto.rs`'s overview-mode roster** for the same staleness. (`src/main.rs` log corrected to 16 tools including `journal`. `src/tools/howto.rs`'s roster `tools` vec was missing `test_notification`; added. `tests/tool_schemas.rs`'s separate synthetic 14-tool list — not named in this checklist item but the same staleness pattern — was also corrected to 16.)
- [ ] **Pre-merge reconciliation task (R4):** deliberately **not performed** in this pass, per D11's own instruction that it is landed "as its own reviewed change — not folded silently into the upgrade commit." `src/tools/maintenance.rs` in this branch touches only the `handle_corrections_bridge` self-call construction (Phase 3's `CallToolRequestParams` fix); the live Studio worktree's separate, pre-existing dirty edits to that file and to `CHANGELOG.md`/`GEMINI.md`/`README.md`/`docs/AGENTS*.md`/`.serena/*`/`history.txt`/`src/bin/kg_debug_tool.rs` were never read, diffed, or imported by this branch. The reconciliation diff itself remains an open task for whoever merges this branch to `master`.
- [ ] **Deployed Host-allowlist evidence (R5):** **not available.** No production install/deploy occurred in this pass (explicitly out of scope — "stop before production install"), so there is no deployed `SURR_HTTP_ALLOWED_HOSTS` value to record yet. This becomes a required Phase 9 deployment-gate artifact.
- [x] Record implementation notes and exact changed paths here. (See Implementation Notes below.)
- [ ] Request CC implementation review before production build/install. (Not requested by this pass — returned as a recommended next step to whoever dispatched this work.)

**Gate:** Source review approved; implementation status may become `Implementation Complete`. Testing remains separate. The pre-merge reconciliation task and the deployed-allowlist evidence are both required inputs to that review, not optional follow-ups. **Both remain open — see the two unchecked items above.** This document's own Status line reads "Implementation Complete (source candidate)" to describe the source diff, not this gate.

## Implementation Notes (this pass)

**Compiler-driven, no improvisation:** every construction-site fix (Tool/CallToolRequestParams/ServerCapabilities/StreamableHttpServerConfig/Implementation/InitializeResult/LoggingMessageNotificationParam) was made by reading the actual rmcp 3.1.4 source in the local cargo registry cache (`~/.cargo/registry/src/*/rmcp-3.1.4/`) for the real constructor/builder API, then verified against the compiler — not guessed from the planning briefing's API sketch.

**Exact changed paths (`git status --porcelain` at completion of this pass):**
```
 M CHANGELOG.md
 M Cargo.lock
 M Cargo.toml
 M README.md
 M docs/AGENTS.md
 M docs/AGENTS/connections.md
 M docs/AGENTS/tools.md
 M src/bin/kg_wander.rs
 M src/config.rs
 M src/http.rs
 M src/main.rs
 M src/server/router.rs
 M src/tools/howto.rs
 M src/tools/maintenance.rs
 M src/tools/test_notification.rs
 M tests/mcp_integration.rs
 M tests/mcp_protocol.rs
 M tests/relationship_smoke.rs
 M tests/test_agent_job_status.rs
 M tests/test_wander.rs
 M tests/tool_schemas.rs
?? tests/stdio_smoke.rs
```
`docs/tasks/20260823-rmcp-3.1.4-upgrade/*.md` (this file and the testing doc) are also edited, tracked separately from the source/test diff above.

**Verification run at completion of this pass (all `--locked` from the first `cargo check` onward, lockfile diff unchanged throughout — same `70 lines / +44/-26` shape every time):**
- `cargo fmt --all -- --check` — exit 0, no diff.
- `cargo check --workspace --all-targets --locked` — exit 0, zero warnings.
- `cargo check --workspace --all-targets --features db_integration --locked` — exit 0, zero warnings.
- `cargo test --workspace --features db_integration --no-run --locked` — exit 0, all targets build (including the new `mcp_protocol.rs` and `stdio_smoke.rs` tests).
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` — exit 0, zero warnings, run repeatedly after every subsequent edit (not just once).
- `cargo test --lib --locked` — 71/71 passed, including all 9 new `http_allowed_hosts_*` tests (10 total `#[test]` functions in `src/config.rs`, the 10th being the pre-existing `test_config_loading`).
- `cargo test --workspace --locked` (default features) — all non-DB-gated tests passed (`relationship_smoke`, `test_agent_job_status`, `tool_schemas`, `workspace_resolution`, `kg_wander`'s unit test, doctests); DB-gated tests correctly no-op/skip without `RUN_DB_TESTS=1`.
- `cargo build --release --locked` — exit 0. Candidate binary SHA-256: `96d375887093f67dc05f33317ca9bc5b5008804019107126f81846ad8872bcd0` (isolated worktree only — never copied toward the live path).

**Not executed this pass (compiled and lint-verified only):** every `RUN_DB_TESTS=1`-gated test — the three new/extended cases in `tests/mcp_protocol.rs`, the `ContentBlock::Text` fixes in `tests/test_wander.rs`/`tests/mcp_integration.rs`, and the new `tests/stdio_smoke.rs`. No verified disposable database/namespace was available in this session, and the task's constraints forbid running database-writing tests against production. This is the honest state of TOOL-01/02/05/06, PROTO-01/02/03/05, and STDIO-01: **written, compiled, lint-clean, protocol-shape-correct by inspection — not proven by execution.**

**Not attempted this pass:** HTTP-01 through HTTP-09 (Host allow/reject/default/error, 4 MiB body boundary, CORS, axum mount) have no test code at all beyond the Phase 1 config-parsing unit tests — see the Known gaps note under Phase 6. This is the single largest remaining verification gap before deployment.

## Phase 9 — Release and deployment gate

- [ ] Complete every pre-deployment case in the testing document.
- [ ] Build the release candidate in the isolated worktree.
- [ ] Copy it to a same-filesystem `.new` path beside the live binary, verify SHA-256 and executable mode, then atomically rename at the supervised gate.
- [ ] Restart only `dev.legacymind.surreal-mind`; do not alter SurrealDB or PhotographyMind.
- [ ] Run live health, public-tunnel, protocol, tools/list, read-tool, and external-client acceptance.
- [ ] Roll back immediately on any failed load-bearing witness, then verify rollback health and public access.

**Gate:** Deployment acceptance passes with rollback preserved. Only then mark the parent complete.

## Non-deliverables

This plan does not implement rmcp tool macros, native task management, output schemas, persistent sessions, MRTR, progress notifications, agent-job repair, dependency unification, or an axum major upgrade unless the 3.1.4 compiler proves axum 0.8 is unavoidable. Any such finding returns to planning rather than entering by opportunistic refactor.
