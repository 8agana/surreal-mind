# SurrealMind rmcp 3.1.4 — Implementation Plan

**Status:** Not Started  
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

- [ ] Add `RuntimeConfig::http_allowed_hosts: Vec<String>`.
- [ ] Parse `SURR_HTTP_ALLOWED_HOSTS` as a comma-separated, trimmed, non-empty list of additional hosts.
- [ ] Always seed the allowlist with `localhost`, `127.0.0.1`, and `::1`; a valid configured list extends and deduplicates that baseline rather than replacing it.
- [ ] Fail startup loudly when the variable is present but empty, contains an empty entry, or otherwise cannot yield a valid allowlist.
- [ ] Add unit tests for absent/default, configured-extension, loopback retention, whitespace, duplicate, empty, and malformed-entry behavior.
- [ ] Add the measured public-origin host to deployment configuration without embedding a machine-only magic constant in Rust.
- [ ] Document the setting in README/AGENTS connection or maintenance documentation.

**Gate:** The production Host value is declared and falsifiable; one production configuration accepts both loopback and the public hostname; no code path silently removes loopback or turns an empty value into allow-all.

## Phase 2 — Dependency bump and compiler inventory

- [ ] Change only `rmcp` from `0.16.0` to exact `3.1.4`; keep all four feature flags. Do not change the `version` field in `Cargo.toml` as a side effect — record that decision explicitly rather than letting it drift (see upgrade doc Constraint 9).
- [ ] Run `cargo check --message-format=short` and save the complete error inventory in the testing document. This first run is expected to move the lockfile; treat that as the one authorized mutation.
- [ ] From this point forward, every subsequent compiler/build/lint command runs with `--locked` (or, where a flag is unavailable, is immediately followed by `git diff Cargo.lock` to confirm zero unexpected drift). The lockfile is established once this step completes; nothing after it should move it again.
- [ ] Run `cargo check --all-targets --features db_integration --message-format=short` after the library reaches a compilable state, so binary and feature-gated failures cannot hide.
- [ ] Record lockfile changes and verify they are limited to the expected rmcp/rmcp-macros/sse-stream and transitive additions or upgrades; use `cargo tree --locked` for post-update dependency evidence.

**Gate:** Compiler evidence supersedes the research counts; unexplained dependency drift blocks the next phase.

## Phase 3 — Migrate model construction

- [ ] Replace all 16 `Tool` literals with `Tool::new(...).with_title(...)` while preserving names, descriptions, input schemas, and ordering.
- [ ] Replace all `CallToolRequestParams` literals with `CallToolRequestParams::new(...).with_arguments(...)`; do not invent `input_responses` or `request_state`.
- [ ] Rebuild `get_info` with supported constructors/builders.
- [ ] Preserve `tools.listChanged = false` explicitly.
- [ ] Keep `ListToolsResult` and `ErrorData` construction unchanged where the compiler confirms they remain exhaustive.
- [ ] Add or update exact assertions for all 16 tools, their order, titles, descriptions, and input schemas.

**Gate:** Library compiler errors from non-exhaustive construction and removed `execution`/`task` fields are zero; tool-surface comparison is exact.

## Phase 4 — Adapt response and content types

- [ ] Change `ServerHandler::call_tool` to return `CallToolResponse`.
- [ ] Convert `CallToolResult` once after router dispatch; leave individual handler return types unchanged.
- [ ] Replace `kg_wander`'s `RawContent::Text`/`.raw` read with `ContentBlock::Text` and the typed text field.
- [ ] Add a focused test proving structured tool results still serialize and a focused test proving `kg_wander` extracts text correctly.

**Gate:** Router, maintenance path, and every build target compile without widening changes into the 15 handler modules.

## Phase 5 — Correct protocol initialization

- [ ] Remove the custom `initialize` echo override after confirming it has no hidden project-specific side effect.
- [ ] Let rmcp negotiate against `supported_protocol_versions`.
- [ ] Narrow `supported_protocol_versions` only if protocol tests prove SurrealMind cannot honor one of rmcp's defaults.
- [ ] Capture initialize responses for a supported legacy version, the current latest supported version, and an unsupported future version.

**Gate:** Unsupported versions are rejected or negotiated according to rmcp's contract; no arbitrary version is echoed as accepted.

## Phase 6 — Port Streamable HTTP deliberately

- [ ] Construct `StreamableHttpServerConfig` from `Default` plus explicit methods for SSE keepalive, legacy-session mode, and allowed hosts.
- [ ] Preserve JSON-response, session-store, and stateless-metadata behavior as decided in the parent document.
- [ ] Leave Origin validation disabled only under parent decision D7 and create the separately assigned security follow-up before this parent closes.
- [ ] Verify the 4 MiB default request cap against observed payload sizes and a synthetic boundary test.
- [ ] Confirm `StreamableHttpService` mounts into axum 0.7 after earlier library errors no longer mask the binary build.
- [ ] Add focused Host allow/reject tests using the exact production hostname and an unlisted control hostname.

**Gate:** Local loopback, measured public Host, rejection control, body-size boundary, and legacy-session tests pass.

## Phase 7 — Preserve notification compatibility without normalizing deprecation

- [ ] Port `LoggingMessageNotificationParam` through its constructor and logger builder.
- [ ] Scope `#[allow(deprecated)]` to the smallest function or module that must retain the public tool.
- [ ] Add a rationale naming SEP-2577 and the separate retirement/replacement task.
- [ ] Verify `test_notification` remains listed and still emits a client-observable notification.

**Gate:** The public tool remains functional and the whole workspace is warning-clean under `-D warnings`.

## Phase 8 — Documentation and implementation review

- [ ] Update `CHANGELOG.md` with the dependency, compatibility decisions, Host allowlist requirement, protocol negotiation correction, and rollback note.
- [ ] Update README/AGENTS material for new environment configuration and acceptance commands.
- [ ] Correct the stale startup log that says 15 tools and omits `journal`, or derive the count/list from the authoritative registry if that is a bounded change. **Also check `src/tools/howto.rs`'s overview-mode roster (the hardcoded `tools` vec around line 23, "Canonical tools roster") for the same staleness** — it is a second hand-maintained list of tool names/one-liners independent of the startup log, and nothing keeps the two in sync automatically.
- [ ] **Pre-merge reconciliation task (R4):** `src/tools/maintenance.rs` and the documentation files already modified in the source Studio worktree at branch-cut time (`CHANGELOG.md`, `GEMINI.md`, `README.md`, `docs/AGENTS.md`, `docs/AGENTS/arch.md`, `docs/AGENTS/connections.md`, `docs/AGENTS/maintenance.md`, plus `.serena/memories/code_conventions.md`, `.serena/project.yml`, `history.txt`, `src/bin/kg_debug_tool.rs` — see `git status` recorded at Phase 0) predate this upgrade and are excluded from it per Constraint 1. Before this branch merges to `master`, diff those live paths against this branch's own edits to the same files (principally `maintenance.rs`, which this upgrade's Phase 3/4 router-boundary work also touches), identify overlap, and land the reconciliation as its own reviewed change — not folded silently into the upgrade commit.
- [ ] **Deployed Host-allowlist evidence (R5):** record the exact string configured for `SURR_HTTP_ALLOWED_HOSTS` at deploy time (launchd plist or canonical env file — whichever is authoritative) verbatim in the implementation-review evidence, even though that destination is not versioned in this repository.
- [ ] Record implementation notes and exact changed paths here.
- [ ] Request CC implementation review before production build/install.

**Gate:** Source review approved; implementation status may become `Implementation Complete`. Testing remains separate. The pre-merge reconciliation task and the deployed-allowlist evidence are both required inputs to that review, not optional follow-ups.

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
