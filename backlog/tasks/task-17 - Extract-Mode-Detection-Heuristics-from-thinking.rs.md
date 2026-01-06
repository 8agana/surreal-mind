---
id: task-17
title: Extract Mode Detection Heuristics from thinking.rs
status: To Do
assignee: []
created_date: '2026-01-06 03:35'
labels:
  - refactoring
  - agent-optimization
  - thinking-rs
  - testability
dependencies:
  - task-25
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract detect_mode() function and keyword lists into dedicated module for single-source-of-truth mode detection.

**Current State**: Mode detection logic exists but heuristics may be duplicated or scattered. Keywords for debug/build/plan/stuck/question/conclude are embedded in detection function.

**Target**: Create src/tools/thinking/mode_detection.rs with testable mode detection logic.

**Impact**: Single source of truth for heuristics; isolated, testable mode detection logic that agents can reuse.

**Implementation Plan**:
1. Extract keyword lists as constants (DEBUG_KEYWORDS, BUILD_KEYWORDS, etc.)
2. Extract detect_mode() function with clear algorithm
3. Create unit tests for mode detection edge cases
4. Update thinking.rs to import detection module

**Acceptance Criteria**:
- mode_detection.rs created with detect_mode(content: &str) → Option<String> function
- Keyword constants exported for visibility
- Unit tests cover mode detection for all 6 modes
- Heuristics documented with examples
<!-- SECTION:DESCRIPTION:END -->
