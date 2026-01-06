---
id: task-18
title: Extract Runners from thinking.rs
status: Done
assignee: []
created_date: '2026-01-06 03:35'
labels:
  - refactoring
  - agent-optimization
  - thinking-rs
  - execution-pattern
dependencies:
  - task-25
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract run_convo() and run_technical() functions (~200 lines) into dedicated module.

**Current State**: Two main execution patterns (conversational vs technical) are embedded in thinking.rs and follow: ThoughtBuilder → framework → memory → result.

**Target**: Create src/tools/thinking/runners.rs with clear runner implementations.

**Impact**: Framework modifications isolated from routing; agents can understand execution flow without reading full thinking.rs.

**Implementation Plan**:
1. Extract run_convo() and run_technical() functions
2. Define shared Runner trait/interface if applicable
3. Move helper functions specific to runners
4. Document the execution flow pattern in module docs

**Acceptance Criteria**:
- runners.rs created with run_convo() and run_technical() public functions
- Both follow consistent execution pattern (builder → framework → memory → result)
- Error handling preserved
- Framework can modify runners behavior without touching main routing logic
<!-- SECTION:DESCRIPTION:END -->
