# SurrealMind rmcp 3.1.4 — Testing and Acceptance

**Status:** Compiler/lint gates (CMP-01..06) executed and passing. Tool-contract, protocol, notification, and stdio tests (TOOL-01/02/05/06, PROTO-01/02/03/05, STDIO-01) are written, compiled, and lint-clean but **not executed** — no verified disposable database was available. HTTP tests (HTTP-01..09) and isolated-runtime tests (RUN-01..07) have **no test code written yet**. Public/external-client acceptance (LIVE-01..08) and rollback (RB-01..03) not started — all require the implementation-review and deployment gates this pass did not reach.  
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
| CMP-05 | Lints | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | Exit 0, no warnings or broad deprecation suppression; lockfile unchanged | **PASS.** Only the two narrow, documented `#[allow(deprecated)]` sites in `src/tools/test_notification.rs` exist anywhere in the diff. |
| CMP-06 | Release build | `cargo build --release --locked` | Exit 0; candidate hash and mode recorded; lockfile unchanged | **PASS.** `target/release/surreal-mind`, Mach-O 64-bit arm64, mode `755`. SHA-256 `96d375887093f67dc05f33317ca9bc5b5008804019107126f81846ad8872bcd0`. Built in the isolated worktree only; never copied toward the live install path. |

## Tool-contract tests

| ID | Test | Expected result |
|---|---|---|
| TOOL-01 | Exact `tools/list` names and order | Same 16 names as baseline; `journal` included; no additions/removals |
| TOOL-02 | Tool metadata | Titles, descriptions, and input schemas equal the 0.16 baseline |
| TOOL-03 | Router conversion | Each handler's `CallToolResult` reaches the client as a complete `CallToolResponse` |
| TOOL-04 | Unknown tool | Method-not-found code and message remain correct |
| TOOL-05 | Text content extraction | `kg_wander` decodes `ContentBlock::Text`; non-text and empty cases fail explicitly |
| TOOL-06 | Notification bridge | `test_notification` remains listed and produces a client-observable notification without workspace warnings |

**Status:** TOOL-01/02 covered by `tests/mcp_protocol.rs::test_list_tools_protocol` (exact 16-name ordered assertion plus a title/description/schema spot-check); TOOL-06 covered by `tests/mcp_protocol.rs::test_notification_protocol_bridge`. TOOL-05 covered by the `ContentBlock::Text` fixes in `tests/test_wander.rs`/`tests/mcp_integration.rs` (the non-text/empty failure-mode half of TOOL-05's wording was not separately exercised — only the success path). All of these are **written, compiled (`cargo test --workspace --features db_integration --no-run --locked`), and lint-clean, but not executed** — no verified disposable database was available this pass. TOOL-03 (router conversion) has no dedicated test beyond the fact that every other DB-gated protocol test's response round-trips through `CallToolResponse` successfully when it eventually runs; TOOL-04 (unknown tool) has no new or existing test found in this pass — both are open gaps.

## Stdio smoke test

Stdio is SurrealMind's default transport (`SURR_TRANSPORT` defaults to `"stdio"`; `main.rs` wires `rmcp::transport::stdio` whenever no other transport is configured), so it gets a runtime witness rather than compile-only coverage (upgrade doc D10). This runs after RUN-01 establishes an isolated candidate identity, using a disposable config and never the live process.

| ID | Test | Expected result |
|---|---|---|
| STDIO-01 | Minimal stdio `initialize`/`tools/list` smoke test | Candidate launched over stdio with a disposable config (never `MCP_NO_LOG` production settings pointed at the live database); send `initialize` then `tools/list` over the pipe; response negotiates a supported protocol version, lists the exact 16-tool contract, and no extraneous (non-protocol) bytes appear on stdout before the process is torn down |

**Status:** Written as `tests/stdio_smoke.rs::stdio01_initialize_and_list_tools_smoke`, gated on `RUN_DB_TESTS=1` (stdio still needs a live `SurrealMindServer::new`, which needs a database, even though the transport itself doesn't). Compiled and lint-clean. **Not executed** — no verified disposable database was available this pass, and this test in particular was never run even once against any database (unlike the `mcp_protocol.rs` cases, which reuse an already-established harness pattern) — treat its process-spawning/framing assumptions (newline-delimited JSON-RPC, matching `tests/test_stdio_persistence.sh`'s established convention for this server) as reviewed-by-inspection, not proven.

## Protocol tests

| ID | Test | Expected result |
|---|---|---|
| PROTO-01 | Supported legacy initialize | Negotiated supported version; legacy session behavior preserved |
| PROTO-02 | Current supported initialize | Negotiated version matches the supported rmcp contract |
| PROTO-03 | Unsupported future initialize | Never echoed as accepted; rejected or negotiated per rmcp contract |
| PROTO-04 | 2026-07-28 lifecycle | Stateless behavior and required metadata match rmcp 3.1.4 semantics |
| PROTO-05 | Capability serialization | `tools.listChanged` remains explicit `false` |

**Status:** PROTO-01/02/03/05 covered by `tests/mcp_protocol.rs::test_initialize_protocol_negotiation`, driving real `initialize` handshakes for `V_2025_03_26`, `ProtocolVersion::LATEST`, and a deserialized synthetic `"2099-01-01"`, and asserting `tools.listChanged == false` on all three responses. STDIO-01 also asserts PROTO-05 independently over the stdio transport. **Written, compiled, lint-clean; not executed** (same DB-availability gap as above). PROTO-04 (2026-07-28 stateless lifecycle) has **no test written** — open gap; this branch does not change `stateless_protocol_metadata_required` (stays `false`, D6), but that decision itself was never exercised against an actual 2026-07-28 client request.

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

**Status: NO TEST CODE WRITTEN for HTTP-01 through HTTP-09.** This is the single largest open gap in this pass, stated plainly rather than papered over. What exists instead: (a) the Phase 1 unit tests in `src/config.rs` exhaustively cover `SURR_HTTP_ALLOWED_HOSTS` *parsing/extension* semantics (which host strings end up in `RuntimeConfig::http_allowed_hosts`), and (b) HTTP-09 is partially covered by inspection — `cargo build --release` succeeds with `axum = "0.7"` unchanged and `StreamableHttpService::new(...)` still mounts via `.nest_service(...)` in `src/http.rs` exactly as before. Neither substitutes for exercising rmcp's actual `Host`-header accept/reject enforcement, the 4 MiB boundary, or CORS behavior against a running `StreamableHttpService`. `start_http_server` builds and binds the whole axum app in one function with no seam to construct just the `Router`/`Service` for an in-process test without either a real bind or a refactor — judged out of scope for a compatibility-preserving pass. **These 9 rows must be executed against a live candidate (RUN-01 preamble) before any deployment decision.**

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

**Status: NOT STARTED.** RUN-01's candidate-on-an-alternate-port preamble was never performed in this pass — all `mcp_protocol.rs`/`stdio_smoke.rs` tests written here use their own isolated mechanisms (`serve_directly` over in-process channels, or a subprocess on stdio) rather than RUN-01's HTTP-candidate-on-alternate-port shape, so none of RUN-02 through RUN-07 as HTTP-runtime rows have been exercised either. This entire section is deployment-gate work for whoever picks up Phase 9.

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

**Status: NOT STARTED.** No production install occurred (out of scope for this pass — "stop before production install"). Requires Phase 0's live-binary/rollback-artifact capture and Phase 9's supervised install, neither performed here.

## Rollback test

| ID | Test | Expected result |
|---|---|---|
| RB-01 | Rollback artifact integrity | Preserved binary hash equals pre-upgrade live hash |
| RB-02 | Rollback procedure rehearsal | Exact commands and targets reviewed before install; no production mutation |
| RB-03 | Actual rollback if acceptance fails | Atomic restore, launchd restart, local health and public MCP restored |

**Status: NOT STARTED.** Phase 0's live-binary SHA-256/rollback-directory copy (required before RB-01 has anything to compare against) was never performed in this pass, consistent with never touching the live Studio worktree or binary.

## Results

**Compiler/lint/build gates (CMP-01..06): all pass**, `--locked` throughout, lockfile stable at one recorded delta. Candidate release binary built and hashed (`96d375887093f67dc05f33317ca9bc5b5008804019107126f81846ad8872bcd0`), never installed.

**Tool-contract, protocol, notification, and stdio tests (TOOL-01/02/05/06, PROTO-01/02/03/05, STDIO-01): written, compiled, and lint-clean; not executed.** No verified disposable database/namespace was available in this session, and running database-writing tests against the production namespace is explicitly forbidden by this task's constraints. This is a real, stated gap, not a soft pass — see the per-section Status notes above for exactly which assertions are proven by inspection versus proven by execution.

**TOOL-03, TOOL-04, PROTO-04: no test written.** Open gaps.

**HTTP-01 through HTTP-09, all Isolated runtime tests (RUN-01..07), all Public/external-client acceptance (LIVE-01..08), and all Rollback tests (RB-01..03): not started.** These require either a live HTTP-serving candidate process or an actual production install, neither of which this pass performed.

## Verdict

**Status:** PARTIAL — compiler/lint/build gates pass; functional test coverage is written but unexecuted; HTTP and live-runtime coverage does not exist yet.  
**Ready for Production:** No
