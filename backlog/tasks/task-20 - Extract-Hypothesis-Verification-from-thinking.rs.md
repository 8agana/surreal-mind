---
id: task-20
title: Extract Hypothesis Verification from thinking.rs
status: To Do
assignee: []
created_date: '2026-01-06 03:35'
labels:
  - refactoring
  - agent-optimization
  - thinking-rs
  - orthogonal-feature
dependencies:
  - task-25
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract run_hypothesis_verification() and related types (~150 lines) into dedicated module.

**Current State**: Hypothesis verification is an orthogonal feature within thinking.rs with its own embedding/scoring logic. Currently embedded in main thinking.rs.

**Target**: Create src/tools/thinking/verification.rs with isolated verification implementation.

**Impact**: Verification work loads ~300 lines instead of 1446; orthogonal feature with clear boundaries.

**Implementation Plan**:
1. Extract run_hypothesis_verification() function
2. Extract verification types and constants
3. Move embedding/similarity scoring logic specific to verification
4. Define public interface for verification mode

**Acceptance Criteria**:
- verification.rs created with run_hypothesis_verification() public function
- Embedding and scoring logic contained within module
- Verification types and configuration accessible
- Existing verification behavior and accuracy preserved
<!-- SECTION:DESCRIPTION:END -->
