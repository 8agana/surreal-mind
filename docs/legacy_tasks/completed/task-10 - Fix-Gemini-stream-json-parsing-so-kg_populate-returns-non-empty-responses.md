---
id: task-10
title: Fix Gemini stream-json parsing so kg_populate returns non-empty responses
status: Completed
assignee: []
created_date: '2026-01-03 04:55'
updated_date: '2026-01-03 04:56'
labels: []
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Problem: kg_populate batches are failing because Gemini responses are empty (0 chars), so JSON extraction cannot be parsed and KG population is blocked.

Root cause analysis: GeminiClient uses `--output-format stream-json` and parses events into `GeminiStreamEvent` expecting `type: "content"` with a `text` field. The current Gemini CLI emits stream-json lines like `{"type":"message","role":"assistant","content":"...","delta":true}` and `{"type":"result",...}`. These no longer match the expected schema, so all content events are ignored and the response buffer stays empty. The fallback parser expects a single JSON object `{session_id,response}` (json output format), which also does not exist in stream-json output, leaving the response empty and breaking kg_populate.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 kg_populate receives non-empty Gemini responses and can parse extraction JSON without the "No JSON found" error.
- [ ] #2 GeminiClient successfully parses current stream-json output (message/result schema) and remains compatible with prior content-based events.
- [ ] #3 When Gemini stream output contains no assistant content, an explicit error is returned with a brief stdout/stderr snippet to aid debugging.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Implementation plan:
1) Update Gemini stream-json event model in `src/clients/gemini.rs` to accept the current schema (add a `Message` variant with fields `role`, `content`, `delta`, plus `Result` for status/stats). Keep the existing `Content` variant for backward compatibility.
2) Extend the stream parser to handle both schemas and to strip optional `data:` prefixes or non-JSON lines; log debug info when a JSON line fails to parse.
3) In the event loop, append assistant message content to `content_buffer` for both `Content{text}` and `Message{role="assistant"}` events (handle `delta` by appending, ignore user/system roles).
4) If the process exits with success but `content_buffer` is empty, return an explicit AgentError (or sentinel) that includes short stdout/stderr snippets, instead of returning an empty response.
5) Add unit tests for stream-json parsing using captured CLI output lines (init/message/result) and legacy content events to ensure non-empty response extraction.

Verification:
- Run `cargo run --bin test_gemini` and confirm `Response:` is non-empty.
- Run `KG_POPULATE_BATCH_SIZE=1 cargo run --bin kg_populate` and confirm no "Raw Gemini response (0 chars)" logs and JSON extraction parses successfully.
- (Optional) Run a small delegate_gemini job and confirm non-empty response + no parse errors in logs.
<!-- SECTION:PLAN:END -->

## Completed and tested.
