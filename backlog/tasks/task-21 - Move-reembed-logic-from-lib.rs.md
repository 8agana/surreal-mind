---
id: task-21
title: Move reembed logic from lib.rs
status: To Do
assignee: []
created_date: '2026-01-06 03:57'
labels:
  - refactoring
  - code-hygiene
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Move run_reembed, run_reembed_kg, run_kg_embed out of lib.rs (~430 lines). These are batch operations, not core library exports. Create src/maintenance/reembed.rs or src/ops/reembed.rs to consolidate reembedding logic and reduce lib.rs scope. Hygiene improvement - functionality works fine where it is, but belongs in a dedicated module.
<!-- SECTION:DESCRIPTION:END -->
