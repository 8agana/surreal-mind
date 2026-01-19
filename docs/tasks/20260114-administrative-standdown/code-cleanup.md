# Code Cleanup Tracking

**Owner:** CC + smcc
**Status:** In Progress
**Parent:** [proposal.md](proposal.md)

---

## Scope

Work derived from [audit-findings.md](audit-findings.md):

- Remove orphaned code
- Standardize patterns
- Fix inconsistencies
- Add missing error handling

---

## Cleanup Tasks

| ID | Task | File(s) | Description | Assignee | Status |
|----|------|---------|-------------|----------|--------|
| CC-01 | Remove Business Logic | `scripts/import_skater_requests.py` | Archive or delete photography script. | CC | **Done** (2026-01-19) |
| CC-02 | Remove Business Logic | `scripts/validate_contacts.py` | Archive or delete photography script. | CC | **Done** (2026-01-19) |
| CC-03 | Consolidate Binaries | `src/bin/kg_*.rs` | Investigate consolidation into CLI commands. | CC | Pending |
| CC-04 | Rename Legacy Tool | `src/tools/delegate_gemini.rs` | Renamed to `call_gem.rs`, handler to `handle_call_gem`. | CC | **Done** (2026-01-19) |
| CC-05 | Rename Legacy Tool | `src/tools/detailed_help.rs` | Renamed to `howto.rs`, handler to `handle_howto`. | CC | **Done** (2026-01-19) |
| CC-06 | Unify Thinking Module | `src/tools/thinking.rs` | Merge/move to `src/tools/thinking/mod.rs` for consistency. | CC | Pending |
| CC-07 | Archive Deprecated Tests | `tests/*.sh` | Review shell scripts vs integration tests. | CC | **Done** (2026-01-19) |

### CC-07 Details

**Deleted** (8 scripts using deprecated `think_search`/`think_convo` tools):
- `simple_test.sh`, `test_with_data.sh`, `debug_search_low_thresh.sh`, `debug_search.sh`
- `test_search.sh`, `test_mcp_comprehensive.sh`, `test_detailed_mcp.sh`, `test_simplified_output.sh`

**Kept** (4 valid scripts):
- `test_simple.sh` - Basic MCP protocol flow test
- `test_mcp.sh` - Basic protocol test
- `test_stdio_persistence.sh` - Valid (uses current `think`/`search` tools)
- `check_version.sh` - Utility script

---

## Pattern Standardization

### Error Handling

**Adopted pattern:** `anyhow::Result` for top-level, custom `SurrealMindError` for logic.

| Location | Current | Target | Status |
|----------|---------|--------|--------|
| `src/tools/*.rs` | Mixed | `SurrealMindError` | Pending |
| `src/bin/*.rs` | `unwrap()` | `anyhow::Context` | Pending |

### Logging

**Adopted pattern:** `tracing::info!`, `warn!`, `error!`. `eprintln!` ONLY for startup panic.

| Location | Current | Target | Status |
|----------|---------|--------|--------|
| `src/main.rs` | Mixed | `tracing` | Pending |
| Legacy Scripts | `print` | `tracing` | Pending |

### Response Formats

**Adopted pattern:** `CallToolResult::structured(json!({...}))`

| Tool | Current | Target | Status |
|------|---------|--------|--------|
| `delegate_gemini` | String? | Structured | Pending |

---

## Verification

After each cleanup:

- [ ] `cargo build --release` succeeds
- [ ] `cargo clippy` clean
- [ ] MCP tools still function (manual test)
- [ ] Relevant tests pass
