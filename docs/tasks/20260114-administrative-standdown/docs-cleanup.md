# Documentation Cleanup Tracking

**Owner:** CC
**Status:** Pending (blocked on Phase 1 audit)
**Parent:** [proposal.md](proposal.md)

---

## Scope

- Update stale documentation
- Remove docs for deleted features
- Ensure core docs are current
- Verify Serena memories match reality

---

## Core Documents

| Document | Location | Current State | Needed Updates | Status |
|----------|----------|---------------|----------------|--------|
| README.md | / | Outdated | Update to reflect current architecture (Modular server, Typed Config). | Pending |
| CHANGELOG.md | / | Unknown | Log "Lobotomy" and v0.7.5 refactor. | Pending |
| AGENTS.md | / | Unknown | Update with current agent roster (`remini`). | Pending |
| docs/AGENTS/tools.md | /docs | Outdated | Remove legacy tools (`delegate_gemini`), add `call_*` tools. | Pending |

---

## Serena Memories

| Memory | Purpose | Matches Reality? | Action | Status |
|--------|---------|------------------|--------|--------|
| project_overview | General Info | TBD | Verify "Business Logic Separation". | Pending |
| code_structure | File map | **NO** | Update with new `src/server`, `src/tools/thinking` split. | Pending |
| code_conventions | Style guide | TBD | Add "Typed Config" and "Anyhow" errors. | Pending |
| suggested_commands | CLI help | **NO** | Remove `import_skater_requests.py` references. | Pending |
| task_completion | History | N/A | Log Audit completion. | Pending |

---

## Task Docs to Review

| Doc | Location | Still relevant? | Action | Status |
|-----|----------|-----------------|--------|--------|
| `docs/tasks/20260115-call_tools/` | `docs/tasks` | Yes | Merge learnings into main docs? | Pending |
| `docs/troubleshooting/*.md` | `docs/troubleshooting` | TBD | Archive if solved. | Pending |

---

## Stale Docs Identified

| Doc | Location | Issue | Resolution | Status |
|-----|----------|-------|------------|--------|
| `scripts/import_skater_requests.py` docs | `scripts/` | Business Logic | Delete with script. | Pending |
| `AGENTS/tools.md` | `docs/AGENTS` | Missing `call_*` | Update. | Pending |
