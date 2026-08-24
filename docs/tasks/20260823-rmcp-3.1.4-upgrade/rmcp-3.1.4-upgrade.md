# SurrealMind rmcp 0.16.0 to 3.1.4

**Status:** Planning Complete — CC approved 2026-08-23  
**Owner:** Codex  
**Reviewer:** CC  
**Repository:** `/Users/samuelatagana/Projects/LegacyMind/surreal-mind` on Studio  
**Source baseline:** `551c1f8ed8aa6b2fbdcd27566624048524dc1bd8` (`master`, SurrealMind 0.8.2)  
**Research briefing:** [`../20260823-rmcp-3.1.4-upgrade-impact.md`](../20260823-rmcp-3.1.4-upgrade-impact.md)  
**Implementation:** [`rmcp-3.1.4-upgrade-impl.md`](rmcp-3.1.4-upgrade-impl.md)  
**Testing:** [`rmcp-3.1.4-upgrade-testing.md`](rmcp-3.1.4-upgrade-testing.md)  
**CC review:** [`rmcp-3.1.4-upgrade-cc-review.md`](rmcp-3.1.4-upgrade-cc-review.md)
**Implementation review:** Codex accepted source HEAD `26d5990` on 2026-08-24 under Sam's directed Fable 5/Sonnet 5 Dynamic Workflow alternative; live CC was not invoked for implementation review

## Plan-review corrections (accepted 2026-08-24)

Landed against the isolated worktree created from `874d229`, not against the still-dirty Studio main worktree. Each item below points to where the correction actually lives; this list is a map, not the specification.

| ID | Correction | Landed in |
|---|---|---|
| R1 | stdio is the default transport (`SURR_TRANSPORT` defaults to `"stdio"` in `config.rs`; `main.rs` wires `rmcp::transport::stdio`) — add a minimal stdio `initialize`/`tools/list` smoke test rather than leaving stdio at compile-only coverage | New `STDIO-01` row, testing doc |
| R2 | RUN-01 (candidate on an alternate port) must execute before every TOOL, PROTO, and HTTP runtime case; those cases must never target the live PID | Isolated runtime tests preamble + RUN-01 row, testing doc |
| R3 | HTTP-03's unlisted-Host control must be evaluated against the same production allowlist as HTTP-01/02, using a control Host with no shared suffix/substring against any allowed entry | HTTP-03 row, testing doc |
| R4 | Add an explicit pre-merge reconciliation task for `src/tools/maintenance.rs` and the pre-existing dirty documentation files; do not import those live edits into this branch now | New Phase 8 task, impl doc |
| R5 | The exact deployed `SURR_HTTP_ALLOWED_HOSTS` value must appear in implementation-review evidence even though the launchd/env destination is unversioned | Phase 8 gate + D5 note, this doc |

Also folded in as cheap, non-blocking corrections: `howto.rs`'s hardcoded tool roster (`src/tools/howto.rs:23`) added to the stale-roster check; `--locked` (or a post-run lockfile re-diff) required on every compiler/build command in the testing doc; intentional warning enforcement stated explicitly rather than assumed; untracked transcript/JSON scratch files (`output.json`, `query_result*.json`, `scratch_query*.json`) named as out-of-scope and never swept into a commit; the `Cargo.toml` package-version decision (stay at `0.8.2` unless explicitly bumped) recorded rather than left to change silently.

## Goal

Upgrade SurrealMind from `rmcp = 0.16.0` to exactly `rmcp = 3.1.4` while preserving its 16-tool public surface, existing legacy-session behavior, public Cloudflare-tunneled endpoint, and rollback path. The migration is complete only when compiler, protocol, local-runtime, public-tunnel, and external-client witnesses all pass.

## Planning evidence

CC's briefing was produced read-only and intentionally ran no Cargo command. Codex independently created a disposable clean archive of commit `551c1f8`, changed only the rmcp version to exact `3.1.4`, and ran `cargo check --message-format=short` on the MBP. The live Studio worktree and binary were untouched.

The first compiler witness produced **40 errors and 16 deprecation warnings** before the binary, HTTP module, build targets, and feature-gated integration tests could be fully reached:

| Surface | Measured result |
|---|---|
| 16 `Tool` literals | 32 errors: non-exhaustive construction plus removed `execution` |
| `get_info` nested literals | 4 non-exhaustive-construction errors |
| `test_notification` | 1 non-exhaustive error plus 16 logging deprecation warnings |
| maintenance self-call | 2 errors: non-exhaustive construction plus removed `task` |
| `call_tool` trait return | 1 type mismatch: `CallToolResult` vs `CallToolResponse` |

The probe also compiler-confirmed that `rmcp::ErrorData` is still re-exported at the crate root and that the existing plain `async fn` trait methods satisfy `MaybeSendFuture` in the default non-`local` build. Upstream 3.1.4 source directly confirms:

- `CallToolResult: Into<CallToolResponse>`;
- `ContentBlock::Text(TextContent)` replaces `RawContent::Text`;
- `Tool::new(name, description, input_schema)` is the supported three-argument constructor;
- `CallToolRequestParams::new(name).with_arguments(...)` is the supported constructor;
- the request-body default is **4 MiB**, resolving the briefing's 4 MiB/10 MB conflict;
- `StreamableHttpServerConfig::default()` enables legacy-session mode but restricts `Host` to loopback values;
- rmcp's default `initialize` negotiates against supported versions instead of echoing the requested version.

## Constraints

1. The Studio worktree was already dirty before planning. No existing modification may be reset, overwritten, absorbed into an upgrade commit, or treated as upgrade evidence.
2. Development occurs in an isolated branch/worktree from a recorded source commit. The live worktree remains the deployment target only at the explicit install gate.
3. No production restart occurs until implementation review and all pre-deployment tests pass.
4. Preserve the four current rmcp feature flags for this branch. Dependency cleanup is not part of the upgrade.
5. Preserve the 16 tool names and schemas unless a separately recorded compatibility decision explicitly says otherwise.
6. Warnings fail the gate. Any temporary deprecation allowance must be narrow, justified, and separately tracked for removal.
7. The current release binary is hashed and copied to a rollback archive before the live path changes.
8. Untracked transcript/scratch JSON files in the source worktree (e.g. `output.json`, `query_result*.json`, `scratch_query*.json`) are out of scope for this upgrade. Never `git add -A`, sweep, or otherwise include them in an upgrade commit or build-evidence diff.
9. The package version in `Cargo.toml` (`0.8.2`) is not changed as a side effect of the dependency bump. If a version bump is warranted, it is a recorded decision with its own rationale, not a silent diff line.

## Decisions

### D1 — Upgrade directly to 3.1.4

The implementation targets the endpoint, not four intermediate shipping releases. The upstream changelog is not reliable enough to make staged major upgrades safer, and no intermediate version will be deployed. Compiler slices remain small and bisectable.

### D2 — Preserve handler internals; adapt at the router boundary

The 15 tool modules continue returning `CallToolResult`. `ServerHandler::call_tool` returns `CallToolResponse` and converts the matched `CallToolResult` exactly once at the router boundary. This avoids changing every handler for a wire-level wrapper type.

### D3 — Replace custom protocol echo with rmcp negotiation

The current `initialize` override accepts and echoes any client-supplied version. Remove that override unless implementation evidence finds project-specific behavior not visible in the current source. Use rmcp's default negotiation and test supported and unsupported versions explicitly. Do not advertise a protocol merely because the SDK knows its name.

### D4 — Preserve capability semantics intentionally

Construct `ServerCapabilities` through its builder. Preserve `tools.listChanged = false` explicitly by mutating `ToolsCapability::default()` and passing it through `enable_tools_with`, rather than silently changing the serialized field from `false` to absent.

### D5 — Configure Host validation; never disable it for convenience

Add an environment-backed `SURR_HTTP_ALLOWED_HOSTS` runtime setting with a fail-closed loopback default. Measure the actual Cloudflare-origin `Host` value before choosing the production value, then declare that value in the launchd environment or canonical environment file. `disable_allowed_hosts()` is not an accepted production solution.

CC resolved the pre-edit measurement with an out-of-band origin witness: one request through the existing `lightroom-demo.samataganaphotography.com -> 127.0.0.1:8080` ingress reached a bounded `nc` listener with `Host: lightroom-demo.samataganaphotography.com`. No Cloudflare `httpHostHeader` override exists. This proves that Cloudflare preserves the original hostname on that ingress shape and strongly supports `mcp.samataganaphotography.com` as the required SurrealMind value. It does not directly measure the `localhost:8787` rule, so public-candidate acceptance remains the final witness.

Configuration semantics are explicit: the secure loopback set (`localhost`, `127.0.0.1`, `::1`) is always retained. An **unset** `SURR_HTTP_ALLOWED_HOSTS` uses that set alone; a valid configured value **extends and deduplicates** it rather than replacing it. A **present but empty or malformed** value fails startup loudly. It must never silently become loopback-only after an operator attempted to add a public host, remove local MCP access, or become allow-all.

**R5:** Because the deployed value lives in the launchd environment or a canonical environment file — neither of which is versioned in this repository — the exact string configured for `SURR_HTTP_ALLOWED_HOSTS` at deploy time must still be captured as evidence in the Phase 8 implementation review (see impl doc). Evidence rules do not relax merely because the value's storage location is outside git.

### D6 — Preserve non-security HTTP semantics unless the upgrade forces otherwise

Set legacy-session mode and SSE keepalive explicitly. Keep JSON-response preference false, the session store absent, and stateless-protocol metadata enforcement false for this migration. Record the 4 MiB body cap and verify existing payload sizes fit with margin; add a separate configuration field only if measurement shows a real need.

### D7 — Defer Origin validation as an explicit security decision

rmcp 3.1.4 defaults `allowed_origins` to empty, disabling Origin validation. Preserve that default during this compatibility migration because the current accepted client-Origin set has not been measured, while the newly explicit Host allowlist and existing bearer authentication still constrain the endpoint. This is a temporary security posture, not merely compatibility inertia. Create a separate follow-up task to measure real client `Origin` headers, define an allowlist, add rejection controls, and decide whether to enable the sibling defense after the upgrade stabilizes.

### D8 — Keep `test_notification` as a compatibility bridge

Do not delete a public tool during a dependency upgrade. Port its non-exhaustive construction through the provided constructor and use the narrowest possible `#[allow(deprecated)]` scope, with a comment naming SEP-2577. Create a separate follow-up task to retire or replace the tool; the upgrade itself must remain warning-clean.

### D9 — Exclude optional rmcp 3.x redesigns

No `#[tool_router]` conversion, output-schema project, native rmcp task-manager adoption, persistent sessions, progress notifications, input-required workflow, dependency unification, or agent-job redesign belongs in this branch. Each may be valuable. None helps answer whether the four-major upgrade works.

### D10 — Stdio gets a runtime smoke test, not compile-only coverage (R1)

`SURR_TRANSPORT` defaults to `"stdio"` (`config.rs`) and `main.rs` starts the server over `rmcp::transport::stdio` whenever no transport is configured — stdio is the default production path, not a secondary one. A candidate that only compiles under stdio and is never exercised at runtime could still ship a stdio-specific regression (e.g. a stray non-protocol byte on stdout, or a negotiation mismatch) undetected. The testing doc adds a minimal `STDIO-01` smoke test: launch the candidate over stdio on a disposable config, send `initialize` then `tools/list`, and assert a clean response with no extraneous stdout bytes before the process is torn down. Compile-only coverage is accepted only if a future codebase change makes runtime stdio execution unsafe to script (e.g. a hard TTY dependency) — no such evidence exists today.

### D11 — Pre-merge reconciliation task for pre-existing dirty state (R4)

The source Studio worktree carries pre-existing modifications to `src/tools/maintenance.rs` and several documentation files (see Constraint 1) that predate this upgrade and are unrelated to it. This branch does not import those live edits — the isolated worktree was created from the recorded commit `874d229c8a4cd7494d452b9d44fa6d56c2e85ffb`, which does not contain them, and they stay that way for the duration of this upgrade. A dedicated pre-merge reconciliation task (Phase 8) records what those edits are, whether they conflict with the upgrade's own changes to `maintenance.rs`, and how they get reviewed and landed separately before or alongside this branch merges back to `master`.

## Risk register

| Risk | Failure shape | Required control |
|---|---|---|
| Public Host rejected | Clean build, every tunneled MCP request returns 403 | Origin-side Host measurement, explicit allowlist, public-tunnel acceptance |
| Origin defense remains disabled | Browser-origin requests are not filtered by rmcp | Explicit temporary decision; bearer auth and Host allowlist retained; separate measured follow-up |
| Protocol falsely accepted | Server echoes a version it does not implement | rmcp negotiation, unsupported-version test, handshake capture |
| Session behavior drifts | New clients become stateless while old clients expect sessions | Legacy and 2026-07-28 protocol tests; session-header assertions |
| Hidden targets remain broken | Library compiles while binary or feature tests do not | `--all-targets`, `db_integration --no-run`, then isolated runtime tests |
| Deprecated logging survives invisibly | Clean build only because warnings were ignored globally | Narrow allowance only; Clippy with `-D warnings`; follow-up task |
| Dirty worktree contaminates change | Unrelated edits enter commits or build evidence | Isolated worktree; path-scoped review; source commit recorded |
| Candidate works locally but not publicly | Local Host and transport differ from Cloudflare | Public hostname handshake and tool call before acceptance |
| Rollback rebuild fails | Old source/dependencies no longer reproduce quickly | Preserve and hash the currently served binary before install |

## Completion boundary

Planning is complete when CC approves this design or all review blockers are resolved. Implementation is complete only when the implementation checklist is satisfied. The upgrade is complete only after the separate test plan passes and a supervised deployment proves the public endpoint and external clients, with the rollback artifact still intact.
