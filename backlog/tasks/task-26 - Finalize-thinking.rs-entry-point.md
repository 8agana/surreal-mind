---
id: task-26
title: Finalize thinking.rs as thin entry point
status: To Do
assignee: []
created_date: '2026-01-06 04:02'
labels:
  - refactoring
  - agent-optimization
  - thinking-rs
  - cleanup
dependencies:
  - task-16
  - task-17
  - task-18
  - task-19
  - task-20
  - task-25
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
After all extractions complete, finalize thinking.rs as the thin entry point (~100-150 lines).

**Current State**: After tasks 16-20 and 25 complete, thinking.rs should contain only:
- `handle_legacymind_think()` entry point (delegates to router)
- `ThoughtBuilder` struct and impl (may stay or move to builders.rs)
- Re-exports for public API

**Target**: Clean, well-documented entry point that serves as the public face of the thinking module.

**Impact**: Agents understand at a glance how legacymind_think works without reading 1446 lines.

**Implementation Plan**:
1. Audit what remains after extractions
2. Decide: keep ThoughtBuilder in thinking.rs or extract to builders.rs
3. Add comprehensive module-level documentation
4. Ensure all public exports are intentional
5. Update any broken imports

**Acceptance Criteria**:
- thinking.rs is ≤200 lines
- Module documentation explains the thinking system architecture
- ThoughtBuilder location is intentional (documented reason to stay or move)
- All tests pass
- Public API unchanged
<!-- SECTION:DESCRIPTION:END -->
