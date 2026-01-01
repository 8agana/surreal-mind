---
id: task-4
title: Fix gemini-cli integration crash (Yoga-layout error)
status: Done
assignee: []
created_date: '2026-01-01 00:30'
updated_date: '2026-01-01 02:15'
labels:
  - bug
  - gemini-cli
  - integration
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The kg_populate binary was blocked by a Node.js/Yoga-layout crash when invoking gemini-cli. This occurs even in non-interactive mode because the CLI still attempts to load the Ink rendering engine.

The fix involves:
1. Modifying the GeminiClient to pass environment variables that disable interactive/colored output: `CI=true`, `TERM=dumb`, and `NO_COLOR=1`.
2. Ensuring `--output-format json` and `-y` (non-interactive) flags are passed.
3. Removing the `PersistedAgent` wrapper for batch jobs to prevent prompt bloat (which exacerbated the race condition).

Refer to doc-6 for execution logs and verification results.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 #1 GeminiClient implementation updated to include environment variables (CI, TERM, NO_COLOR)
- [x] #2 #2 GeminiClient implementation updated to include --output-format json flag
- [x] #3 #3 Verified that kg_populate binary no longer crashes with exit status 13
- [x] #4 #4 Integration test confirms successful extraction of 1 thought batch via updated CLI call
<!-- AC:END -->
