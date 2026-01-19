---
date: 2025-12-31
last_updated: 2025-12-31
prompt_type: Troubleshooting
justification: Fixing serialization error in fire_and_forget job creation
status: Resolved
related_docs:
  - docs/prompts/20251230-delegate_gemini-async.md
  - docs/troubleshooting/20251230-delegate_gemini-async-no-tools.md
---

# delegate_gemini Async Serialization Error

## Error
```
Database error: Serialization error: invalid type: enum, expected any valid JSON value
```

## Red Herrings (What Didn't Fix It)
1. Wrapping `timeout_ms` in `Some()` - wrong problem
2. Adding `Serialize` derive to `JobStatus` enum - wrong problem  
3. Verifying `.as_str()` on JobStatus - already correct, wrong problem

## Root Cause
The error message said "serialization" but the actual failure was **deserialization**.

In `create_job_record()`, `complete_job()`, and `fail_job()` functions, the code was calling `.take(0)?` to deserialize the CREATE/UPDATE response:

```rust
let _: Vec<serde_json::Value> = response.take(0)?;
```

The problem: SurrealDB returns the created/updated record, which includes `exchange_id` with type `Option<Record>`. SurrealDB's `Record` type cannot deserialize to `serde_json::Value`.

## The Fix
Remove the `.take(0)?` deserialization. The query just needs to execute - we don't need the response:

```rust
// Before (broken)
let response = db.query(sql).bind(...).await?;
let _: Vec<serde_json::Value> = response.take(0)?;

// After (working)
db.query(sql).bind(...).await?;
```

Applied to all three functions in `src/tools/delegate_gemini.rs`.

## Lesson Learned
Error messages can be misleading. "Serialization error" actually meant deserialization. Tracing the actual code path reveals the truth - don't just pattern-match on error keywords.

## Verification
```bash
# Test fire_and_forget
delegate_gemini with fire_and_forget=true → returns job_id immediately

# Check job status
agent_job_status with job_id → shows queued → running → completed

# Full lifecycle confirmed working
```
