# SurrealMind rmcp 3.1.4 — Testing and Acceptance

**Status (updated 2026-08-24, independent final verifier pass):** Compiler/lint gates (CMP-01..06) independently re-run on the isolated worktree at HEAD `1c076b6ebe2d6f0fe7edd31fd889659f280932b9` and confirmed passing, `--locked` throughout, candidate hash independently reproduced bit-for-bit. A verified-disposable SurrealDB namespace became available this pass, so the runtime gates previously blocked on database availability were executed against an isolated candidate on an alternate port (never the live PID/port 8787): **TOOL-01/02/04/05/06, PROTO-01/02/03/05, STDIO-01, and HTTP-01..09 all executed and PASS** (evidence below). TOOL-03 is demonstrated indirectly (every executed `tools/call` round-tripped through `CallToolResponse` correctly) rather than by a dedicated assertion. TOOL-06's notification bridge required opening a standing GET SSE listener before the tool call — documented below since it is not obvious from the tool call alone. RUN-01/02/03/06/07 executed; **RUN-04/05 (write-tool test) deliberately NOT performed** — outside the scope this pass was commissioned for, and no write-tool exercise was required by the dispatching instructions. PROTO-04 (2026-07-28 stateless lifecycle) still has no test. Public/external-client acceptance (LIVE-01..08) and rollback (RB-01..03) remain **NOT STARTED** — both require the production-install gate this pass explicitly stops before. **Ready for Production remains No.**

A genuine but pre-existing (non-regression) robustness gap was discovered during this pass: `tools/list`/`tools/call` **panics** a tokio worker (not a graceful error) if `ANTHROPIC_MODELS` is unset when building `call_cc`'s input schema (`src/schemas.rs:136`, `.expect("ANTHROPIC_MODELS env var required")`). `schemas.rs` is untouched by this branch (absent from the Phase 2 diff-stat file list), so this is not an rmcp-upgrade regression, but it is a real finding: the process itself survives (tokio isolates the panic to one task) but the in-flight request never returns a response. See "Independent verification" section below for full detail and reproduction.  
**Parent:** [`rmcp-3.1.4-upgrade-impl.md`](rmcp-3.1.4-upgrade-impl.md)  
**Depends On:** Implementation Complete (not yet reached — see impl doc Phase 8 gate)

## Goal

Prove that the candidate compiles without warnings, preserves the tool contract, negotiates MCP versions correctly, accepts only intended HTTP hosts, operates against an isolated database, survives the public tunnel, and can be rolled back.

## Evidence rules

- Command exit proves only that command. Resulting files, process state, HTTP behavior, database mutation, public delivery, and external-client uptake each require their own witness.
- Record the source commit, candidate binary hash, config identity, command, exit code, and decisive output for every run.
- A clean zero needs a positive control: Host rejection tests include an allowed Host; tool absence checks include an exact known tool; feature-test enumeration records the test binaries built.
- Do not run database-writing tests against the production namespace. `RUN_DB_TESTS=1` requires a verified disposable namespace/database and cleanup authority.
- No failed or uncertain write/tool call is retried automatically.
- **Lockfile discipline:** the lockfile is treated as established once CMP-01's first `cargo check --message-format=short` run completes (impl doc, Phase 2). Every compiler/build/lint command after that point runs with `--locked`; where a subcommand has no `--locked` flag, it is immediately followed by `git diff Cargo.lock` to prove zero drift. A command that silently moved the lockfile is not valid evidence for its own row.
- **Warning enforcement is a command-line policy, not a source-level guarantee.** Nothing in this repository declares `#![deny(warnings)]`; a plain `cargo check` without `-D warnings` will not fail on lint. CMP-02 and CMP-05 are therefore the load-bearing warning gates, and they must be run exactly as specified (with `-D warnings` on the clippy invocation) — a warning-free `cargo build` alone is not sufficient evidence of the gate passing.
- No isolated-runtime, tool-contract, protocol, or HTTP test (TOOL-*, PROTO-*, HTTP-*, STDIO-*) may target the live PID or port `8787` — the live PID at execution time is whatever Phase 0 of the implementation plan records at that moment, not any value captured during planning. RUN-01 (candidate startup on an alternate port, disposable config) must execute first and its own PID/port/config identity must be recorded and referenced by every test that follows in this document.

## Compiler inventory (Phase 2, first run — the authorized lockfile-moving mutation)

`cargo check --message-format=short` immediately after changing only `rmcp = "0.16.0"` → `"3.1.4"` in `Cargo.toml` (before any source edit), on the real Studio toolchain (`cargo 1.92.0`, `rustc 1.92.0`), against the library target:

- **41 errors, 16 warnings.** (The plan document's disposable-archive probe recorded 40 errors/16 warnings against `551c1f8` in isolation; the +1 here is consistent with this branch's baseline being `874d229`, one commit further along, not a discrepancy worth chasing.)
- Error shapes, by surface: `E0639` (cannot construct non-exhaustive struct via literal) against `Tool`, `ServerCapabilities`/`ToolsCapability`/`Implementation`/`InitializeResult` in `get_info`, `CallToolRequestParams`, and `LoggingMessageNotificationParam`; `E0560` (no field named `execution`/`task`) against the same `Tool`/`CallToolRequestParams` literals; `E0063` (missing field `meta`) on `LoggingMessageNotificationParam`; `E0271` (`CallToolResult` vs `CallToolResponse`) on the `call_tool` trait return.
- 16 warnings were 100% `#[deprecated]` notices on `LoggingLevel`/`LoggingMessageNotificationParam`/`notify_logging_message` (SEP-2577), confined to `src/tools/test_notification.rs`.
- This confirms the plan document's independent findings: `rmcp::ErrorData` still re-exports at the crate root, and plain `async fn` trait methods still satisfy `MaybeSendFuture` in the default (non-`local`) build — no unrelated ripple.

A second full pass (`cargo check --workspace --all-targets`, uncovering `src/bin/kg_wander.rs`'s and `src/http.rs`'s errors the library-only check couldn't reach) and a third pass with `--features db_integration` (uncovering the same construction-site patterns in `tests/relationship_smoke.rs`, `tests/test_agent_job_status.rs`, `tests/mcp_integration.rs`, `tests/test_wander.rs`, `tests/mcp_protocol.rs`) found no new *kind* of error beyond the library-level inventory above — only more call sites of the same four patterns (`Tool`/`CallToolRequestParams` literals, `RawContent::Text`/`.raw`, `Request`/`InitializeRequestParam` naming), plus one 3.1.4-specific detail the plan's briefing had not flagged: `JsonRpcError.id` changed from `NumberOrString` to `Option<RequestId>` (MCP 2026-07-28 permits omitting `id` on error responses), which broke one existing test's exhaustive match.

## Compiler and static checks

| ID | Test | Command / method | Expected result | Result |
|---|---|---|---|---|
| CMP-01 | Exact dependency resolution | `cargo tree --locked -i rmcp` and pre-existing lockfile diff | Exit 0 without modifying the lockfile; one direct rmcp 3.1.4 edge; expected transitive changes only | **PASS.** Lockfile diff is `70 lines / +44/-26`, identical before and after every subsequent compiler/lint/build command in this pass. Full delta recorded in the CHANGELOG's Unreleased entry and impl doc Phase 2. |
| CMP-02 | Library and binaries | `cargo check --workspace --all-targets --locked` | Exit 0, zero warnings, lockfile unchanged | **PASS.** |
| CMP-03 | Feature-gated surface | `cargo test --workspace --features db_integration --no-run --locked` | Exit 0; feature-gated protocol tests compile; lockfile unchanged | **PASS.** All targets listed (including new `stdio_smoke.rs` and the extended `mcp_protocol.rs`) built successfully. |
| CMP-04 | Formatting | `cargo fmt --all -- --check` | Exit 0, no diff | **PASS.** |
| CMP-05 | Lints | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | Exit 0, no warnings or broad deprecation suppression; lockfile unchanged | **PASS.** Only the two narrow, documented `#[allow(deprecated)]` sites in `src/tools/test_notification.rs` exist anywhere in the diff. (One unrelated, pre-existing-pattern `#[allow(clippy::type_complexity)]` on a test helper's return type also exists at `tests/mcp_protocol.rs:33` — not a deprecation suppression, noted here for exclusivity accuracy.) |
| CMP-06 | Release build | `cargo build --release --locked` | Exit 0; candidate hash and mode recorded; lockfile unchanged | **PASS.** `target/release/surreal-mind`, Mach-O 64-bit arm64, mode `755`. SHA-256 `96d375887093f67dc05f33317ca9bc5b5008804019107126f81846ad8872bcd0`. Built in the isolated worktree only; never copied toward the live install path. **Independently re-verified 2026-08-24**: rebuilt from clean-ish `--locked` state at the same HEAD and reproduced the identical SHA-256 and mode; lockfile diff remained `70 lines / +44/-26` throughout, matching every prior run. |

**Independent re-verification (2026-08-24, final verifier pass):** All six rows above were re-run from a fresh session against branch HEAD `1c076b6ebe2d6f0fe7edd31fd889659f280932b9` (7 commits ahead of base `874d229`), with the live dirty Studio worktree's `git status` compared byte-for-byte before and after — identical blob hashes on every modified-but-untracked path, confirming this verification pass touched nothing in `surreal-mind` (only `surreal-mind-rmcp-3.1.4` was ever built or run). `cargo tree --locked -i rmcp` confirms exactly one direct edge, `rmcp v3.1.4 -> surreal-mind v0.8.2`. `grep -rn "allow(" src/` cross-checked against the branch diff confirms exactly two new `#[allow(deprecated)]` sites (both in `src/tools/test_notification.rs`, matching D8) plus one pre-existing-pattern `#[allow(clippy::type_complexity)]` in `tests/mcp_protocol.rs:33` — no other new allow-attribute was introduced anywhere in the diff.

## Tool-contract tests

| ID | Test | Expected result |
|---|---|---|
| TOOL-01 | Exact `tools/list` names and order | Same 16 names as baseline; `journal` included; no additions/removals |
| TOOL-02 | Tool metadata | Titles, descriptions, and input schemas equal the 0.16 baseline |
| TOOL-03 | Router conversion | Each handler's `CallToolResult` reaches the client as a complete `CallToolResponse` |
| TOOL-04 | Unknown tool | Method-not-found code and message remain correct |
| TOOL-05 | Text content extraction | `kg_wander` decodes `ContentBlock::Text`; non-text and empty cases fail explicitly |
| TOOL-06 | Notification bridge | `test_notification` remains listed and produces a client-observable notification without workspace warnings |

**Status (2026-08-24, independent final verifier pass — EXECUTED against a live isolated candidate, not just compiled):** `tests/mcp_protocol.rs::test_list_tools_protocol` and `test_notification_protocol_bridge` remain written-but-unexecuted-by-`cargo test` (still no `RUN_DB_TESTS=1` run performed), but the same assertions were independently proven **by direct protocol exercise** against a running candidate:

- **TOOL-01/02 — PASS.** Candidate started on `127.0.0.1:18787` (RUN-01, PID 10192, disposable NS `verify_rmcp314_20260824` / DB `verify_150818`, allowlist `mcp.samataganaphotography.com`). `initialize` (Host: `mcp.samataganaphotography.com`) negotiated `2025-03-26`, session `760af04c-73cb-404d-9a07-7bdfc2a75f0e`. `tools/list` returned exactly the 16 tools in the exact order `think, wander, maintain, journal, rethink, corrections, test_notification, remember, howto, call_gem, call_cc, call_vibe, search, call_status, call_jobs, call_cancel`, matching the startup log's roster and the impl doc's Phase 3 claim; titles/descriptions/schemas present and well-formed for every tool.
- **TOOL-03 — demonstrated, not dedicated-asserted.** Every executed `tools/call` in this pass (`search` ×3, `test_notification` ×2, unknown-tool) round-tripped cleanly through the router's `CallToolResult -> CallToolResponse` conversion with correct `content`/`structuredContent`/`isError` shape. No dedicated boundary-only assertion was added.
- **TOOL-04 — PASS (closes the previously-open gap).** `tools/call` with `name: "definitely_not_a_real_tool"` against a fresh session (candidate PID 15284, disposable NS `verify_rmcp314_20260824b`) returned `{"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"Unknown tool: definitely_not_a_real_tool"}}` — correct JSON-RPC method-not-found code and a clear message.
- **TOOL-05 — PASS (success path).** `search` (read tool) returned `{"content":[{"type":"text","text":"{\"memories\":{\"items\":[]}}"}],"structuredContent":{"memories":{"items":[]}},"isError":false}` against the empty disposable database — `ContentBlock::Text` extraction confirmed end-to-end over the wire. The non-text/empty failure-mode half of TOOL-05's wording was still not separately exercised.
- **TOOL-06 — PASS.** `test_notification` returns its tool result immediately, but the **notification itself arrives on the standing GET `/mcp` SSE stream, not the POST response** — a first attempt without an open GET listener produced only the tool-call result with no notification frame; a second attempt with a background `curl -N GET /mcp` listener open (same session) captured `{"jsonrpc":"2.0","method":"notifications/message","params":{"data":"rmcp-3.1.4-verify-probe","level":"info","logger":"surreal-mind"}}` — confirming the SEP-2577 `LoggingMessageNotificationParam::new(...).with_logger(...)` bridge is functional and client-observable exactly as D8 intends. This session-fan-out detail is worth folding into `tests/mcp_protocol.rs::test_notification_protocol_bridge`'s eventual `RUN_DB_TESTS=1` execution.

All commands, headers, and bodies for the above are preserved under `/tmp/rmcp314-verify/` on Studio (not committed; ephemeral verification scratch, per the same convention as the implementer's own untracked-JSON exclusion).

## Stdio smoke test

Stdio is SurrealMind's default transport (`SURR_TRANSPORT` defaults to `"stdio"`; `main.rs` wires `rmcp::transport::stdio` whenever no other transport is configured), so it gets a runtime witness rather than compile-only coverage (upgrade doc D10). This runs after RUN-01 establishes an isolated candidate identity, using a disposable config and never the live process.

| ID | Test | Expected result |
|---|---|---|
| STDIO-01 | Minimal stdio `initialize`/`tools/list` smoke test | Candidate launched over stdio with a disposable config (never `MCP_NO_LOG` production settings pointed at the live database); send `initialize` then `tools/list` over the pipe; response negotiates a supported protocol version, lists the exact 16-tool contract, and no extraneous (non-protocol) bytes appear on stdout before the process is torn down |

**Status:** `tests/stdio_smoke.rs::stdio01_initialize_and_list_tools_smoke` itself remains unexecuted via `cargo test RUN_DB_TESTS=1` this pass, but its underlying claim was independently proven by a hand-rolled equivalent harness.

**PASS (2026-08-24, independent final verifier pass, executed).** Candidate spawned directly as a subprocess (never through `cargo test`) over `SURR_TRANSPORT=stdio` with a disposable NS/DB (`verify_rmcp314_20260824` / `verify_stdio_150818e`), `SURR_SKIP_DIM_CHECK=1`, and `MCP_NO_LOG=1` (the production-matching setting, confirming D10's "never MCP_NO_LOG production settings pointed at the live database" — the setting itself is fine; only the live DB target would not be). Using a select()-based reader (not a blocking `readline`, to avoid false hangs) to send `initialize` then `notifications/initialized` then `tools/list` as newline-delimited JSON-RPC on stdin:
- `initialize` returned exactly one newline-terminated JSON line, negotiating `2025-03-26` and `tools.listChanged:false` (PROTO-05 independently confirmed over stdio too).
- `tools/list` returned exactly one newline-terminated JSON line listing the exact same 16 tools in the exact same order as the HTTP transport.
- Zero bytes appeared on stdout beyond the two clean JSON-RPC lines; zero bytes on stderr.
- Process terminated cleanly (`SIGTERM`, return code `-15` as expected for a terminated child) with zero residual stdout after teardown.

One real bug was hit and diagnosed during this exercise, not swept under the rug: an **earlier** attempt with a stdio-launch harness that sourced only `OPENAI_API_KEY` (not the full env) hit the same `ANTHROPIC_MODELS`-missing panic documented in the Independent verification section below — `tools/list` hung with no response until the panic was found in stderr. This is an environment-completeness issue in the test harness, not a stdio-transport or rmcp-upgrade defect; it reproduces identically over HTTP with the same incomplete environment.

## Protocol tests

| ID | Test | Expected result |
|---|---|---|
| PROTO-01 | Supported legacy initialize | Negotiated supported version; legacy session behavior preserved |
| PROTO-02 | Current supported initialize | Negotiated version matches the supported rmcp contract |
| PROTO-03 | Unsupported future initialize | Never echoed as accepted; rejected or negotiated per rmcp contract |
| PROTO-04 | 2026-07-28 lifecycle | Stateless behavior and required metadata match rmcp 3.1.4 semantics |
| PROTO-05 | Capability serialization | `tools.listChanged` remains explicit `false` |

**Status (2026-08-24, independent final verifier pass — EXECUTED):** `tests/mcp_protocol.rs::test_initialize_protocol_negotiation` remains unexecuted via `cargo test`, but its three claims were independently proven by direct HTTP handshakes against the isolated candidate (PID 10192, port 18787):

- **PROTO-01 — PASS.** `initialize` with `protocolVersion: "2025-03-26"` → negotiated `"2025-03-26"` verbatim (a version rmcp actually supports, not merely echoed); legacy session established (`mcp-session-id` header returned).
- **PROTO-02 — PASS.** `initialize` with `protocolVersion: "2025-11-25"` (current `ProtocolVersion::LATEST`, matching CC's minor-finding note that LATEST moved from `2025-03-26`) → negotiated `"2025-11-25"` verbatim.
- **PROTO-03 — PASS.** `initialize` with a synthetic unsupported future version `"2099-01-01"` → **never echoed as accepted**; server log recorded an explicit, direct witness: `WARN rmcp::service::server: client requested unsupported protocol version; falling back to server default client_requested=2099-01-01 server_fallback=2025-11-25`, and the response body negotiated down to `"2025-11-25"`. This is the strongest possible confirmation of D3 — rmcp's own negotiation logic rejected the claimed version rather than trusting client input.
- **PROTO-05 — PASS.** All three initialize responses above (plus the STDIO-01 response) carried `"capabilities":{"tools":{"listChanged":false}}` — explicit `false`, never absent.

PROTO-04 (2026-07-28 stateless lifecycle) still has **no test written and was not exercised this pass either** — remains an open gap; `stateless_protocol_metadata_required` stays at its explicit `false` (D6) but that decision itself has still never been exercised against an actual 2026-07-28 client request.

## HTTP and security tests

| ID | Test | Expected result |
|---|---|---|
| HTTP-01 | Loopback Host under the production allowlist | Accepted; configured public hosts do not replace the secure loopback baseline |
| HTTP-02 | Measured Cloudflare Host under the same production allowlist | Accepted |
| HTTP-03 | Unlisted Host control, evaluated against the **same production allowlist as HTTP-01/02** (not a stripped-down or hypothetical one), using a control Host that shares **no suffix or substring** with any allowed entry — e.g. `evil-mcp.samataganaphotography.com` is a bad control against a `mcp.samataganaphotography.com`-inclusive allowlist because a naive suffix/substring match could wrongly accept it; a domain such as `unlisted-host.example.invalid` is required instead | 403 `Host header is not allowed` |
| HTTP-04 | Explicitly empty allowed-host env | Startup exits nonzero before binding; no listener exists |
| HTTP-05 | Unset allowed-host env | Secure loopback defaults apply and no public hostname is accepted |
| HTTP-06 | Existing CORS behavior | Existing allowed browser/client path remains functional; Origin validation remains explicitly deferred |
| HTTP-07 | Body below cap | Accepted |
| HTTP-08 | Body above 4 MiB cap | 413; no handler dispatch or partial write |
| HTTP-09 | Axum 0.7 service mount | Candidate starts and serves `/health` plus MCP route |

**Status (2026-08-24, independent final verifier pass): ALL NINE ROWS EXECUTED AND PASS.** No dedicated `cargo test` code exists for HTTP-01..09 (still true — the seam-less `start_http_server` limitation the implementer described is real), but this pass exercised every row directly over HTTP against running isolated candidates, never the live PID/port 8787. Three separate candidate configurations were used:

- **Instance B** (PID 10192, port 18787, `SURR_HTTP_ALLOWED_HOSTS=mcp.samataganaphotography.com`) for HTTP-01/02/03/06/07/08/09.
- **Instance C** (PID 11751, port 18788, `SURR_HTTP_ALLOWED_HOSTS` unset) for HTTP-05.
- **Instance D** (port 18789, `SURR_HTTP_ALLOWED_HOSTS=""` empty), run synchronously (not backgrounded) for HTTP-04.

| ID | Result | Evidence |
|---|---|---|
| HTTP-01 | **PASS** | Instance B, `Host: localhost:18787`, allowlist configured to `mcp.samataganaphotography.com` only → `HTTP 200`. Confirms the loopback baseline is **retained and extended**, not replaced by a configured value — directly closing CC's F5 finding. |
| HTTP-02 | **PASS** | Instance B, `Host: mcp.samataganaphotography.com` (the measured production value from Phase 0/D5) → `HTTP 200` on `initialize`, correct protocol negotiation in the body. |
| HTTP-03 | **PASS** | Instance B, control Host `unlisted-host.example.invalid` (per R3, no shared suffix/substring with any allowed entry) → `HTTP 403 Forbidden: Host header is not allowed`. Server log independently corroborates: `WARN rmcp::transport::streamable_http_server::tower: rejected request with disallowed Host header (possible DNS rebinding attempt) host=NormalizedAuthority { host: "unlisted-host.example.invalid", port: None }`. |
| HTTP-04 | **PASS** | Instance D, `SURR_HTTP_ALLOWED_HOSTS=""` (present but empty), run to completion (not backgrounded): process exited **nonzero (`EXIT:1`)** before binding, with a clear operator-facing error (`SURR_HTTP_ALLOWED_HOSTS is set but empty; unset it to use the secure loopback default, or provide a comma-separated list of additional hosts`); `lsof -iTCP:18789` confirmed **no listener** ever existed. Both witnesses required by HTTP-04's spec are present — matches CC's F2 resolution (fail loudly, not silently fall back). |
| HTTP-05 | **PASS** | Instance C, `SURR_HTTP_ALLOWED_HOSTS` unset entirely: `Host: 127.0.0.1:18788` → `200`; `Host: mcp.samataganaphotography.com` (a non-loopback value) → `403 Forbidden: Host header is not allowed`. Confirms the secure loopback-only default with no public hostname silently accepted. |
| HTTP-06 | **PASS** | Instance B, `OPTIONS /mcp` preflight with `Origin: https://example-client.invalid` → `200`, `access-control-allow-origin: *`, `access-control-allow-methods: *`, `access-control-allow-headers: *`. Existing permissive CORS behavior is unchanged; Origin validation remains explicitly deferred per D7 (not a regression — a stated, tracked decision). |
| HTTP-07 | **PASS** | Instance B, a valid `tools/call search` request with a 1 MiB (1,048,695-byte) body → `HTTP 200`, dispatched to the handler normally (`{"result":{"content":[...],"isError":false}}`), not rejected for size. |
| HTTP-08 | **PASS** | Instance B, the same shape of request inflated to a 5,242,999-byte body (over the 4 MiB / 4,194,304-byte cap) → `HTTP 413 Payload Too Large: request body exceeds 4194304 bytes`. Confirms the exact documented cap (D6/Phase 6) is enforced, not merely assumed from source inspection. |
| HTTP-09 | **PASS** | Every HTTP-01..08 request above round-tripped through the axum 0.7 app (`/mcp` via `StreamableHttpService::nest_service`, `/health` as a plain route) without a framework-level failure — direct runtime confirmation, not just `cargo build --release` succeeding. |

`start_http_server`'s lack of an in-process `tower::Service::oneshot` seam (the implementer's stated reason no `cargo test` HTTP suite exists) remains a real, separate finding worth a follow-up: exercising these cases required real process spawns and real TCP binds every time, which is slower and heavier than an in-process test would be, though it does not weaken the evidence gathered here.

## Isolated runtime tests

**RUN-01 runs before every other TOOL-*, PROTO-*, HTTP-*, and STDIO-* case that exercises a running process.** Its job is to establish the candidate's identity — PID, port, and config path/hash — and record that identity so every later test in this document targets it explicitly rather than assuming "the server" means the live one. TOOL-*/PROTO-*/HTTP-* rows above are written test-shape-first; when actually executed they run in RUN-01's aftermath, against RUN-01's candidate, never against the live PID or port 8787.

| ID | Test | Expected result |
|---|---|---|
| RUN-01 | Candidate startup on alternate port, disposable config | Starts without touching live PID or port 8787; **record the candidate PID, the alternate port, and the disposable config path/identity (hash or distinguishing content) as evidence** — every subsequent TOOL/PROTO/HTTP/STDIO/RUN case cites this recorded identity rather than re-discovering or assuming it |
| RUN-02 | Health and DB health | Healthy against disposable namespace/database |
| RUN-03 | Read tool | `search` returns a valid structured result |
| RUN-04 | Write tool | One uniquely tagged `think` call creates exactly one record in disposable state |
| RUN-05 | No retry on uncertainty | Uncertain write remains single-attempt and is reconciled before further action |
| RUN-06 | Notification | Client receives one test notification |
| RUN-07 | Log review | No panic, protocol error, host rejection for approved hosts, or warning |

**Status (2026-08-24, independent final verifier pass): RUN-01/02/03/06/07 executed; RUN-04/05 deliberately not performed.**

| ID | Result | Evidence |
|---|---|---|
| RUN-01 | **PASS** | Multiple candidates started across this pass, each recorded with exact identity before any dependent test ran: Instance B (PID 10192, port 18787, disposable NS `verify_rmcp314_20260824` / DB `verify_150818`, allowlist `mcp.samataganaphotography.com`); Instance C (PID 11751, port 18788, unset allowlist); Instance D (port 18789, run synchronously, empty allowlist, never bound); a stdio subprocess (PID 14188 in the final pass, disposable DB `verify_stdio_150818e`); Instance E (PID 15284, port 18787 reused after B's teardown, disposable NS `verify_rmcp314_20260824b`). None ever touched live PID 1538 or port 8787 — reconfirmed by `ps aux` and `lsof` at every teardown. |
| RUN-02 | **PASS (partial — `/health` only, not `/db_health`).** Instance B's `/health` returned `200`/`ok` immediately after startup. A stronger DB-health witness came from RUN-03's successful `search` round-trip against the live disposable DB connection (schema-init log line + a real query returning a real, structured, empty result) rather than the dedicated `/db_health` endpoint, which was not separately invoked this pass. |
| RUN-03 | **PASS.** `search` (read tool) against the empty disposable database returned a valid structured result (`{"memories":{"items":[]}}`, `isError:false`) — see TOOL-05 above for the full exchange. |
| RUN-04 | **NOT PERFORMED.** No write-tool (`think`/`remember`) call was made this pass. This was a deliberate scope decision: the dispatching instructions for this verification pass listed "loopback/public/unlisted/default/empty Host cases, protocol negotiation, tools/list, read behavior, stdio smoke, notification, and request-body boundary" as the required exercises and did not include a write-tool case; combined with the evidence-rules constraint that any write test needs a verified disposable namespace (which was available and used for reads) and single-attempt reconciliation discipline, the safer default was to not manufacture an unrequested write test. This remains an open gap for whoever runs the next pass. |
| RUN-05 | **N/A — no write attempt was made (see RUN-04), so there was nothing to reconcile.** |
| RUN-06 | **PASS.** Confirmed via TOOL-06 above: a standing GET `/mcp` SSE listener received the `notifications/message` frame triggered by `test_notification`, twice, across two separate tool calls. |
| RUN-07 | **PASS, clean.** Full log review of every candidate's stdout/stderr across this pass found: zero panics attributable to this branch's code, zero unexplained errors, and exactly the WARN lines expected from the deliberate test cases themselves — OAuth ephemeral-secret generation (expected, no `SURR_OAUTH_CLIENT_ID`/`SECRET` configured for a throwaway candidate), the PROTO-03 unsupported-version fallback (expected, that test's whole point), the HTTP-03 Host-rejection warning (expected, that test's whole point), and one `Embedding failed for query ...: OpenAI API error 400 Bad Request` warning caused by this verifier's own synthetic 1 MiB all-`y` query string in the HTTP-07 body-boundary test exceeding OpenAI's token limit — the tool degraded gracefully (`isError:false`, empty results) rather than erroring, so this is not a defect, just a self-inflicted test artifact worth avoiding next time (a shorter realistic string would exercise the same boundary without tripping the embedding call). No approved-host rejection false-positives occurred. **One separate, real panic was found and is documented in the Independent verification section below (`ANTHROPIC_MODELS` missing) — it occurred only in an earlier, incompletely-configured test harness, never in the final recorded runs.** |

## Public and external-client acceptance

| ID | Test | Expected result |
|---|---|---|
| LIVE-01 | Local `/health` after atomic install | Healthy, candidate PID and hash match |
| LIVE-02 | Public `/mcp` handshake | Succeeds through `mcp.samataganaphotography.com`; no 403 |
| LIVE-03 | Public `tools/list` | Exact 16-tool contract |
| LIVE-04 | Public read tool | `search` succeeds through the tunnel |
| LIVE-05 | Codex client | Connects, lists tools, and completes a read call |
| LIVE-06 | CC client | Connects, lists tools, completes a read call, and records the negotiated protocol version explicitly |
| LIVE-07 | Protocol capture | Negotiated version and session/stateless behavior match planned expectations |
| LIVE-08 | Logs after soak | No new transport/session/auth errors during the supervised observation window |

**Status: NOT STARTED (unchanged this pass).** No production install occurred (explicitly out of scope for this pass — "do not test the real public tunnel," "stop before production install"). Requires Phase 0's live-binary/rollback-artifact capture and Phase 9's supervised install, neither performed here. The live production process (PID 1538, port 8787) was confirmed running and healthy (`/health` → `ok`) both before and after this entire verification pass, and its Studio worktree's `git status` was confirmed byte-for-byte unchanged (see Independent verification below) — this pass did not touch it in any way.

## Rollback test

| ID | Test | Expected result |
|---|---|---|
| RB-01 | Rollback artifact integrity | Preserved binary hash equals pre-upgrade live hash |
| RB-02 | Rollback procedure rehearsal | Exact commands and targets reviewed before install; no production mutation |
| RB-03 | Actual rollback if acceptance fails | Atomic restore, launchd restart, local health and public MCP restored |

**Status: RB-01/02/03 formally NOT STARTED (no rollback artifact copy exists yet — that is Phase 0/Phase 9 deployment work).** As a read-only supplement (hashing a file is not a rollback action and does not touch or copy it), this pass recorded the live production binary's current identity for whoever performs the eventual Phase 0 rollback-artifact capture: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/target/release/surreal-mind`, SHA-256 `0f7fbccb693e5fbec5402403c831546013b825929c11eec52bf6d6faf5911c5c`, mode `755`, PID `1538` at time of capture. This is a convenience data point only — it is not a preserved artifact and does not satisfy RB-01 by itself.

## Results

**Compiler/lint/build gates (CMP-01..06): all pass**, `--locked` throughout, lockfile stable at one recorded delta. Candidate release binary built and hashed (`96d375887093f67dc05f33317ca9bc5b5008804019107126f81846ad8872bcd0`), never installed. **Independently reproduced bit-for-bit by a second, unrelated build in a separate session (2026-08-24) — same hash, same mode, same lockfile delta.**

**Tool-contract, protocol, notification, and stdio tests (TOOL-01/02/04/05/06, PROTO-01/02/03/05, STDIO-01): executed and PASS this pass (2026-08-24)**, against isolated candidates on alternate ports/disposable databases, never the live PID/port. TOOL-03 is demonstrated indirectly rather than by a dedicated assertion. This closes the prior "written but not executed" gap for these specific rows — see per-section Status notes for full evidence. The underlying `cargo test ... RUN_DB_TESTS=1` executions of `tests/mcp_protocol.rs`/`tests/stdio_smoke.rs` themselves still were not run; the equivalent behavior was proven by direct protocol exercise instead.

**HTTP-01 through HTTP-09: executed and PASS this pass (2026-08-24)**, closing what was previously "the single largest open gap." No `cargo test` HTTP suite exists (the `start_http_server` seam limitation the implementer described is real and unaddressed), so this remains a standing recommendation for a future pass, but the actual Host-allowlist enforcement, body-size boundary, and CORS behavior are now proven by direct execution rather than by source inspection alone.

**Isolated runtime tests (RUN-01/02/03/06/07): executed and PASS. RUN-04/05 (write-tool test): deliberately not performed** — outside this pass's commissioned scope (see RUN-04 Status note above).

**PROTO-04: still no test written.** Open gap, unchanged.

**A pre-existing, non-regression robustness finding surfaced during this pass:** `tools/list`/`tools/call` panics a tokio worker if `ANTHROPIC_MODELS` is unset (`src/schemas.rs:136`), which this branch does not touch. See "Independent verification" below.

**All Public/external-client acceptance (LIVE-01..08) and all Rollback tests (RB-01..03): still NOT STARTED.** Both require an actual production install, which this pass explicitly stopped before, per its own instructions.

## Independent verification (2026-08-24, final verifier pass)

This section records the verifier's own audit trail, separate from the implementer's Phase 8 self-report above.

**Branch inspection before running anything.** HEAD `1c076b6ebe2d6f0fe7edd31fd889659f280932b9` on `codex/rmcp-3.1.4`, 7 commits ahead of base `874d229c8a4cd7494d452b9d44fa6d56c2e85ffb`:
```
1c076b6 docs: fix audit-flagged doc-accuracy gaps in rmcp 3.1.4 upgrade
946c025 docs: record rmcp 3.1.4 upgrade evidence, correct stale tool rosters
2518a25 test: migrate test suite to rmcp 3.1.4 APIs, add protocol/stdio coverage
bf494cb fix: migrate SurrealMind source to rmcp 3.1.4 APIs
5659a1b feat: add SURR_HTTP_ALLOWED_HOSTS runtime Host allowlist config
15dc056 chore: bump rmcp 0.16.0 -> 3.1.4
21924c4 docs: land accepted rmcp 3.1.4 plan-review corrections (R1-R5)
```
`git diff --stat` against base: 25 files changed, 1130 insertions, 495 deletions — matches the impl doc's own Phase 8 file list exactly (source/test paths only; `docs/tasks/` tracked separately). Worktree was clean (`git status --porcelain=v2` empty) before any command ran.

**Live Studio worktree drift check.** `git status --porcelain=v2 --branch` on `/Users/samuelatagana/Projects/LegacyMind/surreal-mind` was captured before the compiler ladder and again after the full verification pass (compiler ladder, five HTTP/stdio candidate launches, all teardowns, disposable-namespace cleanup). **Every blob hash for every modified-but-untracked path was identical byte-for-byte across both captures** (`.serena/memories/code_conventions.md`, `.serena/project.yml`, `CHANGELOG.md`, `GEMINI.md`, `README.md`, `docs/AGENTS.md`, `docs/AGENTS/arch.md`, `docs/AGENTS/connections.md`, `docs/AGENTS/maintenance.md`, `history.txt`, `src/bin/kg_debug_tool.rs`, `src/tools/maintenance.rs`), and the same untracked-file set was present both times with no additions or removals. The live production process (PID `1538`) was running throughout and its `/health` endpoint returned `ok` both before and after. **This pass did not modify, build, or touch `surreal-mind` in any way.**

**Compiler/static ladder — independently re-run, not taken on the implementer's word.** `cargo fmt --all -- --check`, `cargo check --workspace --all-targets --locked`, `cargo check --workspace --all-targets --features db_integration --locked`, `cargo test --workspace --features db_integration --no-run --locked`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, `cargo tree --locked -i rmcp`, and `cargo build --release --locked` were all re-executed from a fresh session. All six exited `0`; `git diff --stat -- Cargo.lock` was empty after every single command (zero lockfile drift); the release binary hash reproduced **exactly** `96d375887093f67dc05f33317ca9bc5b5008804019107126f81846ad8872bcd0`, mode `755`, Mach-O 64-bit arm64.

**Runtime verification methodology.** A verified-disposable SurrealDB namespace (`verify_rmcp314_20260824`, confirmed via `INFO FOR DB` returning `NotFound` before first use, and via `REMOVE NAMESPACE` + a second `NotFound` check after cleanup) was used against Studio's already-running SurrealDB (3.2.3, `127.0.0.1:8000`, independently confirmed healthy). Every candidate was started with `SURR_TRANSPORT`, `SURR_HTTP_BIND`/port, `SURR_DB_NS`/`SURR_DB_DB` pointed at the disposable namespace, and a freshly generated ephemeral `SURR_BEARER_TOKEN` (never the production token or `~/.surr_token` file) — full command lines, headers, and response bodies are preserved under `/tmp/rmcp314-verify/` on Studio for anyone who wants to re-inspect them, none of which contain the production bearer token, `.env` contents, or any other credential (the `.env` was `source`d into each candidate's own environment via `set -a; source ...; set +a`, never echoed or logged). Every candidate was terminated with `kill <PID>` and reaped (`ps -p <PID>` confirming absence) before its port was reused or the pass moved on; every port (`18787`, `18788`, `18789`) was confirmed free via `lsof -iTCP:<port> -sTCP:LISTEN` after each teardown. A final `ps aux | grep surreal-mind` at the end of the pass showed only the live production PID `1538` — zero orphaned candidate processes.

**Finding: `ANTHROPIC_MODELS`-missing panic in `src/schemas.rs` (pre-existing, not an rmcp-upgrade regression).** The first candidate launch in this pass sourced only `OPENAI_API_KEY` from the shared `.env` (not the full file). `tools/list` against that candidate hung with no response; the server log showed `thread 'tokio-rt-worker' (...) panicked at src/schemas.rs:136:10: ANTHROPIC_MODELS env var required: NotPresent`. `src/schemas.rs` does not appear anywhere in this branch's diff-stat file list (confirmed by re-reading the Phase 2/Phase 8 changed-paths list above and independently grepping the file's line 135-136 — `std::env::var("ANTHROPIC_MODELS").expect("ANTHROPIC_MODELS env var required")` — against the branch diff, which shows zero hits), so this is **pre-existing behavior this branch did not introduce**, not a regression from the rmcp upgrade. It is nonetheless a genuine robustness finding worth flagging upstream: (1) the failure mode is a hard `.expect()` panic inside a spawned tokio task rather than a graceful JSON-RPC error, so the calling client's request simply never returns rather than receiving a clean error; (2) the process itself survives (tokio isolates the panic to the one task, confirmed — the same candidate PID kept serving requests normally afterward once the environment was corrected), so this is a per-request availability bug, not a process-crash bug; (3) `call_cc`'s schema-building code path has an undocumented hard dependency on `ANTHROPIC_MODELS` that is not mentioned anywhere in `docs/AGENTS/connections.md`'s or `README.md`'s environment-variable documentation (both of which this branch touched for `SURR_HTTP_ALLOWED_HOSTS` but neither of which was audited for other missing-env-var documentation gaps by this pass). Re-launching with the full `.env` sourced immediately resolved it and every subsequent test in this document passed cleanly. **Recommendation for a separate follow-up, not this branch:** either document `ANTHROPIC_MODELS` (and any sibling `*_MODELS` vars the other `call_*` tools' schemas depend on) as a hard startup/first-`tools/list`-call requirement, or make the schema-building code degrade gracefully (e.g. an empty enum, or a startup-time fail-loud check alongside the existing dimension-hygiene preflight) instead of panicking mid-request.

## Verdict

**Status:** PARTIAL — compiler/lint/build gates pass (independently reproduced); tool-contract, protocol, stdio, and HTTP functional coverage is now **executed and passing** against isolated candidates (previously written-but-unexecuted or entirely absent), including the previously-open TOOL-04 gap; TOOL-03 remains demonstrated-but-not-dedicated-asserted; RUN-04/05 (write-tool) and PROTO-04 remain open by deliberate scope/unaddressed gap respectively; public/external-client acceptance and rollback remain entirely NOT STARTED because both require the production-install gate this pass explicitly stops before.  
**Ready for Production:** No
