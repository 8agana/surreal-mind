# REMini scheduler deduplication

**Date:** 2026-09-03  
**CLU case:** `fed-74ee30`  
**Decision:** Keep `dev.legacymind.nightly-shift`; preserve-disable the standalone `dev.legacymind.remini` LaunchAgent.

## Problem

Both LaunchAgents were loaded at 01:00 and each invoked `remini --all`. They
performed duplicate provider and knowledge-graph work and raced to overwrite
`logs/remini_report.json`.

## Decision basis

`nightly-shift` is the intentional superset: Phase 1 invokes REMini with an
explicit 1800-second task timeout, then Phase 2 performs bounded Antigravity
reflection. Its Phase 2 completed with exit 0 on the four recorded nights
through 2026-09-03. The recurring wander failure was the same child-level
headless permission failure under both schedulers, not a nightly-shift-specific
failure.

Sam delegated the survivor decision to CC and Codex in `fed-676dec`. CC proposed
keeping nightly-shift; Codex independently measured the live definitions and
logs and accepted in `fed-74ee30` comment 148. Sam then closed `fed-676dec` and
cleared execution in `fed-e63f66` comment 150.

## Scope

This change alters scheduler topology only. It does not rebuild or deploy any
binary, edit REMini behavior, run REMini, mutate SurrealDB, or change the
nightly-shift LaunchAgent.
