# legacymind_update implementation

**Date**: 2025-12-26
**Prompt Type**: Implementation Plan (New Tool)
**Justification**: Creating tools for Gemini CLI to use when called by surreal-mind. Solving issues with memories_populate and future tools. 
**Status**: Implemented
**Implementation Date**: 2025-12-27
**Previous Connected Prompt**: docs/prompts/20251221-memories_populate-implementation.md

___

## Summary
Implemented a new `legacymind_update` tool to update existing thoughts (including `extracted_to_kg`) in a direct, tool-visible way. This enables Gemini CLI (and other tool users) to perform safe, auditable updates without relying on hidden orchestration inside `memories_populate`.

## Why
`memories_populate` repeatedly failed to persist the boolean update on thoughts despite multiple query variants. This tool creates a clean, reusable update path so Gemini (or any tool user) can:
- Mark thoughts as processed (`extracted_to_kg`, `extraction_batch_id`, `extracted_at`)
- Update continuity fields (chain/session links)
- Adjust metadata (tags, status, significance, etc.)
- Update content (with optional re-embedding)

This simplifies future tooling by reusing a single, tested update mechanism across workflows.

## Tool Contract (Inputs)
`legacymind_update` accepts:
- `thought_id` (string; accepts `thoughts:<id>` or raw id)
- `updates` (object; allowed fields listed below)
- `reembed` (bool, default true; only used when content is changed)

Allowed update fields:
- `content`, `tags`, `chain_id`, `session_id`, `previous_thought_id`, `revises_thought`, `branch_from`
- `extracted_to_kg`, `extraction_batch_id`, `extracted_at`
- `status`, `significance`, `injection_scale`, `access_count`, `last_accessed`, `submode`
- `framework_enhanced`, `framework_analysis`, `origin`, `is_private`, `confidence`

## Behavior
- Normalizes `tags` to string array (accepts string, array, or null).
- If `content` is updated and `reembed` is not false, recomputes embedding + metadata.
- If `extracted_to_kg` is set true and `extracted_at` is omitted, auto-sets `extracted_at` to now.
- Supports UUID-typed and string-typed IDs by parsing UUID when possible.
- Returns a structured response: `updated` boolean, `fields_updated`, and `reembedded`.

## Implementation Details
Files changed:
- `src/tools/legacymind_update.rs` (new handler + validation + update logic)
- `src/tools/mod.rs` (module export)
- `src/schemas.rs` (input/output schemas + added to detailed_help)
- `src/server/router.rs` (tool registration + dispatch)

## Next Steps
- Use `legacymind_update` inside `memories_populate` for marking thoughts processed.
- Add prompt/docs showing Gemini workflow with this tool.

___

## Testing Results

**See docs/troubleshooting/20251226-legacymind_update-troubleshooting.md**

___

**Status**: Implemented - Troubleshooting
**Implementation Date**: 2025-12-26
**Connected Prompt Docs**:
**Troubleshooting Docs**: 
- [pending] docs/troubleshooting/20251226-legacymind_update-troubleshooting.md
**Reference Doc**: 
**Closure Notes**:
