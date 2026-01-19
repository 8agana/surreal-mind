# Test Coverage Tracking

**Owner:** CC + smcc
**Status:** In Progress
**Parent:** [proposal.md](proposal.md)

---

## Scope

- Audit existing test coverage
- Add tests for uncovered edge cases
- Ensure all MCP tools have verification tests
- Document test procedures

---

## MCP Tool Test Coverage

| Tool | Has Tests? | Happy Path | Error Cases | Edge Cases | Status |
|------|------------|------------|-------------|------------|--------|
| think | **YES** | `integration_test.rs`, `test_stdio_persistence.sh` | Unit tests in `thinking.rs` | Unit tests in `mode_detection.rs` | Active |
| search | **YES** | `unified_search.rs` (unit), `test_search_mcp.json` | - | - | Needs Review |
| remember | TBD | | | | Pending |
| wander | **YES** | `test_wander.rs` | - | - | Active |
| maintain | TBD | | | | Pending |
| call_gem | **YES** | `test_gemini_call.rs`, `gemini_client_integration.rs` | - | - | Active |
| call_cc | **YES** | Unit tests in `call_cc.rs` | - | - | Active |
| call_codex | **YES** | Unit tests in `call_codex.rs` | - | - | Active |
| call_status | **YES** | `test_agent_job_status.rs` | - | - | Active |
| call_jobs | TBD | | | | Pending |
| call_cancel | TBD | | | | Pending |
| rethink | TBD | | | | Pending |
| corrections | TBD | | | | Pending |
| howto | TBD | | | | Pending |

---

## Binary Test Coverage

| Binary | Has Tests? | Notes | Status |
|--------|------------|-------|--------|
| surreal-mind (server) | **YES** | `test_simple.sh`, `test_mcp.sh`, `mcp_protocol.rs` | Core server logic tested via integration |
| kg_populate | **NO** | Orphan binary (consolidating) | - |
| kg_embed | **NO** | Orphan binary (consolidating) | - |
| gem_rethink | **NO** | Orphan binary (consolidating) | - |
| remini | **NO** | Supervisor logic untested | - |

---

## Test Procedures

### Automated Tests

Run the full suite:

```bash
cargo test
```

Run MCP integration verification (requires server build):

```bash
./tests/test_simple.sh
./tests/test_stdio_persistence.sh
```

### Manual MCP Tool Verification

```bash
# Template for manual verification
# Tool: [name]
# Date: [date]
# Tester: [who]

# Test 1: [description]
# Expected: [result]
# Actual: [result]
# Status: PASS/FAIL
```

---

## Missing Tests to Add

| ID | Component | Test Case | Priority | Assignee | Status |
|----|-----------|-----------|----------|----------|--------|
| TC-01 | `remember` | Memory persistence verification | High | CC | Pending |
| TC-02 | `maintain` | DB optimization logic | Medium | CC | Pending |
| TC-03 | `corrections` | KG correction flow | Medium | CC | Pending |
| TC-04 | `rethink` | Rethink marker logic | Medium | CC | Pending |
| TC-05 | `remini` | Supervisor loop (mocked time) | High | CC | Pending |

---

## Test Infrastructure Notes

- **Unit Tests**: Co-located in `src/`. Run via `cargo test`.
- **Integration Tests**: `tests/*.rs`. Run via `cargo test`.
- **Shell Tests**: `tests/*.sh`. End-to-end blackbox tests against the binary.
- **Framework**: Standard Rust `test` harness + `tokio::test` for async.
