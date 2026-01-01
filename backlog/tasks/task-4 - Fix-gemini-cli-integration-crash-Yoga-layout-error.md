---
id: task-4
title: Fix gemini-cli integration crash (Yoga-layout error)
status: To Do
assignee: []
created_date: '2026-01-01 00:30'
labels:
  - bug
  - gemini-cli
  - integration
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The kg_populate binary is currently blocked by a Node.js/Yoga-layout crash when invoking gemini-cli. This occurs even in non-interactive mode because the CLI still attempts to load the Ink rendering engine.

The fix involves:
1. Modifying the GeminiClient to pass `--output-format json` to all CLI calls.
2. Updating the output parser to handle the resulting JSON structure.
3. Verifying the fix resolves the 'unsettled top-level await' error in subprocess environments.

Refer to doc-6 for execution logs of the crash.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 #1 GeminiClient implementation updated to include --output-format json flag
- [ ] #2 #2 GeminiClient response parsing updated to handle JSON output format (extracting 'response' field)
- [ ] #3 #3 Verified that kg_populate binary no longer crashes with exit status 13
- [ ] #4 #4 Integration test confirms successful extraction of 1 thought batch via updated CLI call
<!-- AC:END -->
