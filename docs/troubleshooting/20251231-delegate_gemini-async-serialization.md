---
date: 2025-12-31
last_updated: 2025-12-31
prompt_type: Troubleshooting
justification: Fixing enum serialization error in fire_and_forget job creation
status: Complete
related_docs:
  - docs/prompts/20251230-delegate_gemini-async.md
  - docs/troubleshooting/20251230-delegate_gemini-async-no-tools.md
---

# delegate_gemini async serialization fix (JobStatus)

## Problem
fire_and_forget job creation fails with:
```
Database error: Serialization error: invalid type: enum, expected any valid JSON value
```

## Root Cause
JobStatus was bound into SurrealDB queries as a Rust enum. The SurrealDB bind
serializer only accepts JSON scalar/array/object values, so it rejected the
enum during agent_jobs inserts/updates.

## Fix Applied
- Added serde behavior on JobStatus to serialize as a lowercase string
  (queued, running, completed, failed, cancelled).
- Updated delegate_gemini job CREATE/UPDATE queries to bind $status using the
  JobStatus enum instead of embedding string literals.

## Verification
1. Run `cargo check` to validate compilation.
2. Call delegate_gemini with `fire_and_forget=true` and confirm:
   - Job row is created with status=queued.
   - Status transitions to running/completed or failed.
