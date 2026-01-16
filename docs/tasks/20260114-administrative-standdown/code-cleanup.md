# Code Cleanup Tracking

**Owner:** CC + smcc
**Status:** Pending (blocked on Phase 1 audit)
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
| | | | | | |

---

## Pattern Standardization

### Error Handling
**Adopted pattern:** *TBD after audit*

| Location | Current | Target | Status |
|----------|---------|--------|--------|
| | | | |

### Logging
**Adopted pattern:** *TBD after audit*

| Location | Current | Target | Status |
|----------|---------|--------|--------|
| | | | |

### Response Formats
**Adopted pattern:** *TBD after audit*

| Tool | Current | Target | Status |
|------|---------|--------|--------|
| | | | |

---

## Verification

After each cleanup:
- [ ] `cargo build --release` succeeds
- [ ] `cargo clippy` clean
- [ ] MCP tools still function (manual test)
- [ ] Relevant tests pass
