---
id: task-16
title: Extract Mode Router from thinking.rs
status: To Do
assignee: []
created_date: '2026-01-06 03:35'
labels:
  - refactoring
  - agent-optimization
  - thinking-rs
  - cognitive-load
dependencies:
  - task-25
  - task-17
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract mode routing logic (~230 lines) from handle_legacymind_think into dedicated module.

**Current State**: Mode routing is embedded in main function with ~230 lines of repetitive pattern matching for debug/build/plan/stuck/question/conclude modes.

**Target**: Create src/tools/thinking/mode_router.rs with clean mode dispatch logic.

**Impact**: Agents modifying mode behavior load ~350 lines instead of 1446 lines of thinking.rs.

**Implementation Plan**:
1. Extract match statement routing each mode to appropriate handler
2. Define clear input/output contracts for mode dispatch
3. Move mode constants/configuration to mode_router module
4. Update thinking.rs to import and delegate to router

**Acceptance Criteria**:
- mode_router.rs created with route_by_mode() public function
- All mode-specific pattern matching moved from thinking.rs
- Existing behavior preserved (no test failures)
- Module loads cleanly when imported by other agents
<!-- SECTION:DESCRIPTION:END -->
