---
id: task-23
title: Add cognitive engine tests
status: Done
assignee: []
created_date: '2026-01-06 03:57'
labels:
  - testing
  - cognitive-engine
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CognitiveEngine::blend() has zero test coverage. Add unit tests in cognitive/mod.rs with #[cfg(test)] module. Test edge cases: empty weights, all-zero weights, dedup behavior, proportional allocation correctness. Builds confidence in core blending logic used across contemplation and thinking workflows.
<!-- SECTION:DESCRIPTION:END -->
