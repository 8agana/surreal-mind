---
id: task-12
title: Add shadow graph injection + explicit writes for Gemini
status: To Do
assignee: []
created_date: '2026-01-04 04:07'
labels:
  - surreal-mind
  - gemini
  - shadow-graph
  - memory
  - delegate-tools
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create a shared "shadow graph" memory layer for Gemini: semantic injection based on prompt and explicit writes captured from tool output. Default injection on. Shared across agents but only Gemini uses for now.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Shadow graph tables/schemas created (entities/observations/edges) or unified table with type field
- [ ] #2 delegate_gemini performs semantic retrieval from shadow graph and injects compact context by default
- [ ] #3 delegate_gemini supports tuning (top_k, min_sim, scope) via params/env defaults
- [ ] #4 Explicit shadow writes are parsed from Gemini output and persisted; malformed blocks cause tool failure
- [ ] #5 Shadow search/write tools exposed for manual use (if desired)
- [ ] #6 Docs updated with usage + defaults
<!-- AC:END -->
