---
date: 2025-12-30
prompt type: Implementation Plan (Tool Refactor)
justification: To make the delegate_gemini tool fire and forget instead of blocking.
status: Complete
implementation date: 2025-12-30
prompt doc: docs/prompts/20251230-delegate_gemini-async.md
previous doc: docs/prompts/20251230-delegate_gemini-async-grok.md
related_docs:
  - docs/prompts/20251227-gemini-cli-implementation.md
  - docs/prompts/20251228-delegate-gemini-tool.md
  - docs/prompts/20251230-delegate-gemini-cwd.md
  - docs/prompts/20251230-delegate-gemini-timeout.md
research dos: 
  - docs/research/20251230-delegate_gemini-async-grok.md
---

## Codex review

### Summary
- Agree with fire-and-forget via background task.
- Not complete as written due to schema/persistence gaps.

### Key gaps
1. Output schema requires response/session_id/exchange_id; async ack cannot satisfy.
2. No durable job tracking; job_id needs storage + lookup.
3. agent_exchanges is SCHEMAFULL and requires response; no placeholder row.
4. PersistedAgent ignores session_id when calling underlying agent; resume broken.
5. Unbounded tokio::spawn can flood; add semaphore/queue.

### Suggested minimal changes
- Add agent_jobs table (status, tool_name, created_at, started_at, completed_at, error, session_id, exchange_id, metadata).
- Add status tool or DB query path by job_id.
- Update delegate_gemini output schema to oneOf: sync result or queued response.
- Fix PersistedAgent to pass session_id through and store resume_session_id metadata.
- Consider a concurrency limiter for background tasks.

### Open questions
- Should async be opt-in flag or default?
- Should job tracking live in tool_sessions or a new table?

---
**status**: Complete
**implementation date**: 2025-12-30
**prompt doc**: docs/prompts/20251230-delegate_gemini-async.md
**previous doc**: docs/prompts/20251230-delegate_gemini-async-grok.md
**related docs**:
  - docs/prompts/20251227-gemini-cli-implementation.md
  - docs/prompts/20251228-delegate-gemini-tool.md
  - docs/prompts/20251230-delegate-gemini-cwd.md
  - docs/prompts/20251230-delegate-gemini-timeout.md
**research docs**:
  - docs/research/20251230-delegate_gemini-async-grok.md
**next docs**:
  - docs/prompts/20251230-delegate_gemini-async.md
