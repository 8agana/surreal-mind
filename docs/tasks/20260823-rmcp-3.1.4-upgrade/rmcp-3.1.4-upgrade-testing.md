# SurrealMind rmcp 3.1.4 — Testing and Acceptance

**Status:** Not Started  
**Parent:** [`rmcp-3.1.4-upgrade-impl.md`](rmcp-3.1.4-upgrade-impl.md)  
**Depends On:** Implementation Complete

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

## Compiler and static checks

| ID | Test | Command / method | Expected result |
|---|---|---|---|
| CMP-01 | Exact dependency resolution | `cargo tree --locked -i rmcp` and pre-existing lockfile diff | Exit 0 without modifying the lockfile; one direct rmcp 3.1.4 edge; expected transitive changes only |
| CMP-02 | Library and binaries | `cargo check --workspace --all-targets --locked` | Exit 0, zero warnings, lockfile unchanged |
| CMP-03 | Feature-gated surface | `cargo test --workspace --features db_integration --no-run --locked` | Exit 0; feature-gated protocol tests compile; lockfile unchanged |
| CMP-04 | Formatting | `cargo fmt --all -- --check` | Exit 0, no diff |
| CMP-05 | Lints | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | Exit 0, no warnings or broad deprecation suppression; lockfile unchanged |
| CMP-06 | Release build | `cargo build --release --locked` | Exit 0; candidate hash and mode recorded; lockfile unchanged |

## Tool-contract tests

| ID | Test | Expected result |
|---|---|---|
| TOOL-01 | Exact `tools/list` names and order | Same 16 names as baseline; `journal` included; no additions/removals |
| TOOL-02 | Tool metadata | Titles, descriptions, and input schemas equal the 0.16 baseline |
| TOOL-03 | Router conversion | Each handler's `CallToolResult` reaches the client as a complete `CallToolResponse` |
| TOOL-04 | Unknown tool | Method-not-found code and message remain correct |
| TOOL-05 | Text content extraction | `kg_wander` decodes `ContentBlock::Text`; non-text and empty cases fail explicitly |
| TOOL-06 | Notification bridge | `test_notification` remains listed and produces a client-observable notification without workspace warnings |

## Stdio smoke test

Stdio is SurrealMind's default transport (`SURR_TRANSPORT` defaults to `"stdio"`; `main.rs` wires `rmcp::transport::stdio` whenever no other transport is configured), so it gets a runtime witness rather than compile-only coverage (upgrade doc D10). This runs after RUN-01 establishes an isolated candidate identity, using a disposable config and never the live process.

| ID | Test | Expected result |
|---|---|---|
| STDIO-01 | Minimal stdio `initialize`/`tools/list` smoke test | Candidate launched over stdio with a disposable config (never `MCP_NO_LOG` production settings pointed at the live database); send `initialize` then `tools/list` over the pipe; response negotiates a supported protocol version, lists the exact 16-tool contract, and no extraneous (non-protocol) bytes appear on stdout before the process is torn down |

## Protocol tests

| ID | Test | Expected result |
|---|---|---|
| PROTO-01 | Supported legacy initialize | Negotiated supported version; legacy session behavior preserved |
| PROTO-02 | Current supported initialize | Negotiated version matches the supported rmcp contract |
| PROTO-03 | Unsupported future initialize | Never echoed as accepted; rejected or negotiated per rmcp contract |
| PROTO-04 | 2026-07-28 lifecycle | Stateless behavior and required metadata match rmcp 3.1.4 semantics |
| PROTO-05 | Capability serialization | `tools.listChanged` remains explicit `false` |

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

## Rollback test

| ID | Test | Expected result |
|---|---|---|
| RB-01 | Rollback artifact integrity | Preserved binary hash equals pre-upgrade live hash |
| RB-02 | Rollback procedure rehearsal | Exact commands and targets reviewed before install; no production mutation |
| RB-03 | Actual rollback if acceptance fails | Atomic restore, launchd restart, local health and public MCP restored |

## Results

No implementation or acceptance test has run. The only existing compiler evidence is the disposable version-only planning probe recorded in the parent document; it is a baseline, not a candidate verdict.

## Verdict

**Status:** PENDING  
**Ready for Production:** No
