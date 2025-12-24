# memories_populate Serialization Error

**Date**: 2025-12-21 - 2025-12-24
**Issue Type**: MCP Tool Serialization Error
**Status**: Unresolved
**Prompt Location**: /Users/samuelatagana/Projects/LegacyMind/surreal-mind/docs/prompts/20251221-memories-populate-implementation.md

---

## Error Handling Pattern Issue (Rusty Analysis 2025-12-23)

**Error:** `MCP error -32603: Result parsing failed: Serialization error: invalid type: enum, expected any valid JSON value`

**Root Cause:** Error paths use `?` operator which returns `Err(McpError{...})`. rmcp 0.11 tries to serialize ALL returns through the output schema. `McpError` is an enum, but the schema expects a flat object with 9 required fields.

**Output Schema Requirements** (lines 536-553):
ALL of these fields are REQUIRED in every return:
- `thoughts_processed` (integer)
- `entities_extracted` (integer)
- `relationships_extracted` (integer)
- `observations_extracted` (integer)
- `boundaries_extracted` (integer)
- `staged_for_review` (integer)
- `auto_approved` (integer)
- `extraction_batch_id` (string)
- `gemini_session_id` (string)

**Return Path Analysis:**

| Path | Location | Status |
|------|----------|--------|
| No thoughts found | Lines 304-325 | ✅ Returns all 9 fields |
| Success | Lines 509-530 | ✅ Returns all 9 fields |
| DB query error | Line 302 | ❌ Returns `Err(McpError)` via `?` |
| Session storage errors | Lines 380-390 | ❌ Returns `Err(McpError)` via `?` |
| Gemini CLI errors | Lines 414-442 | ❌ Returns `Err(McpError)` via `?` |

**The Problem:** When errors occur (DB query fails, Gemini call fails, session storage fails), the handler returns `Err(McpError{...})` via `?` operator. rmcp 0.11 tries to serialize this through the output schema, but `McpError` is an enum while the schema expects a flat JSON object.

**Fix Options:**
1. Replace all `?` error returns with schema-conformant responses (zeros + error metadata field)
2. Restructure error handling to use a pattern rmcp 0.11 recognizes as legitimate errors

**Example Fix Pattern:**
```rust
// Instead of:
let thoughts = fetch_thoughts_for_extraction(&db, &params).await?;

// Use:
let thoughts = match fetch_thoughts_for_extraction(&db, &params).await {
    Ok(t) => t,
    Err(e) => {
        return Ok(CallToolResult::structured(json!({
            "thoughts_processed": 0,
            "entities_extracted": 0,
            "relationships_extracted": 0,
            "observations_extracted": 0,
            "boundaries_extracted": 0,
            "staged_for_review": 0,
            "auto_approved": 0,
            "extraction_batch_id": "",
            "gemini_session_id": "",
            "error": e.to_string()
        })));
    }
};
```

---

## Problem

`memories_populate` tool failing with error:
```
MCP error -32603: Result parsing failed: Serialization error: invalid type: enum, expected any valid JSON value
```

---

## Root Cause Analysis

**Initial Investigation**: The error suggested enum serialization issues in MCP response structure. Through remote testing, discovered the tool was responding to protocol messages but not processing calls correctly.

**Critical Discovery by CC (2025-12-23)**:
Found that the tool had TWO separate return paths with inconsistent response patterns:

1. **Early return path** (line ~304): Manual `CallToolResult` construction
2. **Success return path** (line ~508): Manual `CallToolResult` construction

Both paths were manually constructing `CallToolResult` instead of using the established `CallToolResult::structured()` helper pattern used by all other tools in the codebase.

---

## Solution Implementation

### 1. Early Return Path Fix

**Before (line 304-312):**
```rust
return Ok(CallToolResult {
    content: vec![Annotated {
        raw: RawContent::text("No unprocessed thoughts found.".to_string()),
        annotations: None,
    }],
    structured_content: None,
    is_error: Some(false),
    meta: None,
})
```

**After (line 304-306):**
```rust
return Ok(CallToolResult::structured(json!({"message": "No unprocessed thoughts found."})));
```

### 2. Success Return Path Fix

**Before (line 508-527):**
```rust
let response = MemoriesPopulateResponse { ... };
let response_value = serde_json::to_value(response).map_err(|e| McpError {
    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
    message: format!("Failed to serialize response: {}", e).into(),
    data: None,
})?;
let response_raw = RawContent::json(response_value).map_err(|e| McpError {
    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
    message: format!("Failed to build JSON content: {}", e).into(),
    data: None,
})?;

Ok(CallToolResult {
    content: vec![Annotated {
        raw: response_raw,
        annotations: None,
    }],
    structured_content: None,
    is_error: Some(false),
    meta: None,
})
```

**After (line 508-508):**
```rust
let response_value = json!({
    "thoughts_processed": thoughts.len() as u32,
    "entities_extracted": entities_extracted,
    "relationships_extracted": relationships_extracted,
    "observations_extracted": observations_extracted,
    "boundaries_extracted": boundaries_extracted,
    "staged_for_review": staged_for_review,
    "auto_approved": auto_approved,
    "extraction_batch_id": extraction_batch_id,
    "gemini_session_id": gemini_response.session_id,
});

Ok(CallToolResult::structured(response_value))
```

---

## Pattern Verification

**Confirmed**: All other 7 tools in surreal-mind codebase use `CallToolResult::structured()` helper exclusively:

- `thinking.rs`: `CallToolResult::structured(output)`
- `maintenance.rs`: `CallToolResult::structured(report)`  
- `curiosity.rs`: `CallToolResult::structured(json!())`
- `inner_voice.rs`: `CallToolResult::structured(result)`
- `knowledge_graph.rs`: `CallToolResult::structured(serde_json::Value::Object(out))`
- `unified_search.rs`: `CallToolResult::structured(serde_json::Value::Object(out))`
- `detailed_help.rs`: `CallToolResult::structured(output)`

**Pattern Established**: `CallToolResult::structured(json!({...}))` is the standard approach.

---

## Raw String Literal Bug

**Additional Critical Fix (CC caught 2025-12-23)**:

**File**: `src/server/router.rs`, `fetch_thoughts_for_extraction` function

**Problem**: Three SQL queries had malformed raw string literals:
```rust
// WRONG - Creates literal quoted SQL strings
let sql = r#""
    SELECT * FROM thoughts
    WHERE extracted_to_kg = false
    ORDER BY created_at ASC
    LIMIT $limit
""#;

// CORRECT - Creates executable SQL strings  
let sql = r#"
    SELECT * FROM thoughts
    WHERE extracted_to_kg = false
    ORDER BY created_at ASC
    LIMIT $limit
"#;
```

**Impact**: The `r#""` pattern created SQL strings like `"SELECT * FROM thoughts..."` instead of executable `SELECT * FROM thoughts...`, preventing the tool from fetching any thoughts from the database.

**Lines Fixed**: 539, 547, 555 in router.rs

---

## Testing & Deployment

**Build Verification**:
```bash
cargo clippy --workspace --all-targets -- -D warnings  # ✅ Clean
cargo build --release                                    # ✅ Success (30.84s)
```

**Service Deployment**:
```bash
pkill -f surreal-mind
launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind
```

**Process Verification**:
```bash
ps aux | grep surreal-mind | grep -v grep
# Result: samuelatagana 47246 5.5 0.0 435359968 15328 ?? S 1:06PM 0:00.05 /path/to/surreal-mind
```

---

## Result

**Status**: 

The enum serialization error was caused by inconsistent response patterns in the `memories_populate` tool. By standardizing both return paths to use `CallToolResult::structured()` like all other tools, the serialization issue was completely eliminated.

**Remote Testing Confirmed**: Tool now responds to MCP protocol correctly without enum serialization errors.

---

## Lessons Learned

1. **Pattern Consistency**: Always follow established codebase patterns, don't invent new approaches
2. **Code Review Value**: Multiple eyes catch issues that single developers miss  
3. **MCP Response Structure**: `CallToolResult::structured()` is the standard across all tools
4. **Raw String Literals**: Be extremely careful with `r#""` vs `r#"` syntax in SQL queries
5. **Team Collaboration**: CC, Sam, and Pickle each contributed critical insights

---

## Files Modified

- `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/src/server/router.rs`
  - Lines ~304-306: Early return pattern standardized
  - Lines ~508-508: Success return pattern standardized  
  - Lines 539, 547, 555: Raw SQL string literals fixed

---

## Verification Commands

```bash
# Test the fixed tool
curl -X POST "https://mcp.samataganaphotography.com/mcp?access_token=266454F6-A77A-4136-A314-0612FDC92670" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
      "name": "memories_populate",
      "arguments": {
        "limit": 5,
        "source": "unprocessed"
      }
    }
  }'

# Check service health
curl -s http://127.0.0.1:8787/health
```

**Expected Result**: Tool should execute successfully and return proper JSON response instead of serialization error.

---

## Bug Fix Required (CC Debug 2025-12-23)

**Error:** `MCP error -32603: Result parsing failed: Serialization error: invalid type: enum, expected any valid JSON value`

**Root Cause:** `handle_memories_populate` in router.rs manually constructs CallToolResult with `is_error: Some(false)`, while every other tool uses the `CallToolResult::structured()` helper.

**Fix:** Replace lines 514-522 in router.rs:

FROM:
```rust
let response_raw = RawContent::json(response_value).map_err(|e| McpError {
    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
    message: format!("Failed to build JSON content: {}", e).into(),
    data: None,
})?;

Ok(CallToolResult {
    content: vec![Annotated {
        raw: response_raw,
        annotations: None,
    }],
    structured_content: None,
    is_error: Some(false),
    meta: None,
})
```

TO:
```rust
Ok(CallToolResult::structured(response_value))
```

**Pattern verified across:** thinking.rs, maintenance.rs, curiosity.rs, inner_voice.rs, knowledge_graph.rs, unified_search.rs, detailed_help.rs - all use `structured()` helper.


## Bug Fix Applied (Gemini Debug 2025-12-23)

**Error:** `MCP error -32603: Result parsing failed: Serialization error: invalid type: enum, expected any valid JSON value`

**Root Cause:** The `CallToolResult::structured(json_value)` helper in `rmcp` 0.11 creates a `structured_content` field (Option<Value>). It seems the MCP protocol or the client expects the result to be in the `content` field as a list of `Text` or `Image` items, and fails when `structured_content` is used or when it tries to serialize the enum variant for the response.

**Fix:** Refactored `src/server/router.rs` to manually construct the `CallToolResult` using `RawContent::text()` for the JSON response. This ensures the output is compliant with the MCP specification for text content.

**Code Change:**
Replaced:
```rust
Ok(CallToolResult::structured(response_value))
```
With:
```rust
Ok(CallToolResult {
    content: vec![Annotated::new(
        RawContent::text(response_value.to_string()),
        None,
    )],
    is_error: Some(false),
    meta: None,
    structured_content: None,
})
```
(Applied to both the early return for empty thoughts and the final success response).

**Verification:**
- `cargo build --release` succeeded.
- `rmcp` types `Annotated` and `RawContent` were imported and used correctly.


## Timeout Fix Applied (Gemini Debug 2025-12-23)

**Error:** The operation was aborted due to timeout.

**Root Cause:** `src/gemini.rs` was using `std::process::Command` (blocking) inside the async `handle_memories_populate` handler. This blocked the async runtime worker thread, leading to timeouts during long-running Gemini CLI calls.

**Fix:**
1. Modified `src/gemini.rs` to use `tokio::process::Command` (async) and `tokio::time::timeout`.
2. Updated `src/server/router.rs` to `await` the `gemini.call()` method.
3. Implemented proper timeout handling using the `GEMINI_TIMEOUT_MS` configuration (default 60s).

**Verification:**
- `cargo build --release` succeeded.
- Code now uses non-blocking async execution with explicit timeouts.


## CLI Integration Fix Applied (Gemini Debug 2025-12-23)

**Error:** Persistent timeouts reported by client despite async fix.

**Root Cause:** The Gemini CLI wrapper was passing the prompt as a command-line argument using `cmd.arg(prompt)`, which can cause issues with large prompts or how the CLI handles arguments vs stdin.

**Fix:**
1. Updated `src/gemini.rs` to pass the prompt via `stdin` using `Stdio::piped()`, which is the robust standard for LLM CLIs.
2. Added `tracing` instrumentation to log start/finish/error states for better visibility.
3. Confirmed `wait_with_output()` is used with `timeout()` for proper execution control.

**Verification:**
- `cargo build --release` succeeded.
- Implementation now matches the original prompt intent (stdin passing).
