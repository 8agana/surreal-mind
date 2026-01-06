---
id: task-19
title: Extract Continuity Resolution from thinking.rs
status: To Do
assignee: []
created_date: '2026-01-06 03:35'
labels:
  - refactoring
  - agent-optimization
  - thinking-rs
  - persistence
dependencies:
  - task-25
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract resolve_continuity_links() and helper functions (~120 lines) into dedicated module.

**Current State**: Continuity resolution logic handles linking to previous_thought_id, branch_from, revises_thought, and chain continuity. Currently embedded in thinking.rs main logic.

**Target**: Create src/tools/thinking/continuity.rs with clean continuity interface.

**Impact**: Continuity work loads ~270 lines instead of 1446; isolated from routing/execution concerns.

**Implementation Plan**:
1. Extract resolve_continuity_links() and helper types
2. Define ContinuityInput and ContinuityResult types
3. Create clean interface: fn resolve(input: ContinuityInput) → Result<ContinuityResult>
4. Move continuity-related database queries

**Acceptance Criteria**:
- continuity.rs created with resolve() public function
- Clear types for continuity inputs and outputs
- Database query logic isolated from core resolution logic
- Existing continuity behavior preserved
<!-- SECTION:DESCRIPTION:END -->
