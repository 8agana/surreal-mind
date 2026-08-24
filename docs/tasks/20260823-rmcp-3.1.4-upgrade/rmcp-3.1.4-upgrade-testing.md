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

## Compiler and static checks

| ID | Test | Command / method | Expected result |
|---|---|---|---|
| CMP-01 | Exact dependency resolution | `cargo tree --locked -i rmcp` and pre-existing lockfile diff | Exit 0 without modifying the lockfile; one direct rmcp 3.1.4 edge; expected transitive changes only |
| CMP-02 | Library and binaries | `cargo check --workspace --all-targets` | Exit 0, zero warnings |
| CMP-03 | Feature-gated surface | `cargo test --workspace --features db_integration --no-run` | Exit 0; feature-gated protocol tests compile |
| CMP-04 | Formatting | `cargo fmt --all -- --check` | Exit 0, no diff |
| CMP-05 | Lints | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Exit 0, no warnings or broad deprecation suppression |
| CMP-06 | Release build | `cargo build --release` | Exit 0; candidate hash and mode recorded |

## Tool-contract tests

| ID | Test | Expected result |
|---|---|---|
| TOOL-01 | Exact `tools/list` names and order | Same 16 names as baseline; `journal` included; no additions/removals |
| TOOL-02 | Tool metadata | Titles, descriptions, and input schemas equal the 0.16 baseline |
| TOOL-03 | Router conversion | Each handler's `CallToolResult` reaches the client as a complete `CallToolResponse` |
| TOOL-04 | Unknown tool | Method-not-found code and message remain correct |
| TOOL-05 | Text content extraction | `kg_wander` decodes `ContentBlock::Text`; non-text and empty cases fail explicitly |
| TOOL-06 | Notification bridge | `test_notification` remains listed and produces a client-observable notification without workspace warnings |

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
| HTTP-03 | Unlisted Host control | 403 `Host header is not allowed` |
| HTTP-04 | Explicitly empty allowed-host env | Startup exits nonzero before binding; no listener exists |
| HTTP-05 | Unset allowed-host env | Secure loopback defaults apply and no public hostname is accepted |
| HTTP-06 | Existing CORS behavior | Existing allowed browser/client path remains functional; Origin validation remains explicitly deferred |
| HTTP-07 | Body below cap | Accepted |
| HTTP-08 | Body above 4 MiB cap | 413; no handler dispatch or partial write |
| HTTP-09 | Axum 0.7 service mount | Candidate starts and serves `/health` plus MCP route |

## Isolated runtime tests

| ID | Test | Expected result |
|---|---|---|
| RUN-01 | Candidate startup on alternate port | Starts without touching live PID or port 8787 |
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
