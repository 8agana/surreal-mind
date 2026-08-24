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

### D6 — Preserve non-security HTTP semantics unless the upgrade forces otherwise

Set legacy-session mode and SSE keepalive explicitly. Keep JSON-response preference false, the session store absent, and stateless-protocol metadata enforcement false for this migration. Record the 4 MiB body cap and verify existing payload sizes fit with margin; add a separate configuration field only if measurement shows a real need.

### D7 — Defer Origin validation as an explicit security decision

rmcp 3.1.4 defaults `allowed_origins` to empty, disabling Origin validation. Preserve that default during this compatibility migration because the current accepted client-Origin set has not been measured, while the newly explicit Host allowlist and existing bearer authentication still constrain the endpoint. This is a temporary security posture, not merely compatibility inertia. Create a separate follow-up task to measure real client `Origin` headers, define an allowlist, add rejection controls, and decide whether to enable the sibling defense after the upgrade stabilizes.

### D8 — Keep `test_notification` as a compatibility bridge

Do not delete a public tool during a dependency upgrade. Port its non-exhaustive construction through the provided constructor and use the narrowest possible `#[allow(deprecated)]` scope, with a comment naming SEP-2577. Create a separate follow-up task to retire or replace the tool; the upgrade itself must remain warning-clean.

### D9 — Exclude optional rmcp 3.x redesigns

No `#[tool_router]` conversion, output-schema project, native rmcp task-manager adoption, persistent sessions, progress notifications, input-required workflow, dependency unification, or agent-job redesign belongs in this branch. Each may be valuable. None helps answer whether the four-major upgrade works.

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
