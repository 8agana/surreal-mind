# delegate_gemini timeout_ms Parameter Implementation

**Date**: 2025-12-30
**Author**: CC
**Status**: Ready for implementation
**Related**: delegate_gemini tool, dynamic timeout handling

## Problem Statement

Currently `delegate_gemini` timeout is only configurable via the `GEMINI_TIMEOUT_MS` environment variable (default 60 seconds). This forces all federation calls through the same timeout, preventing per-call timeout control for:

- Quick queries that should fail fast if Gemini is unresponsive
- Long-running analysis tasks that need extended timeouts
- Network-constrained situations where timeout needs dynamic adjustment

## Solution

Expose `timeout_ms` as an optional tool parameter that allows per-call timeout override while maintaining backward compatibility with the environment variable.

## Changes Required

### 1. DelegateGeminiParams (src/tools/delegate_gemini.rs:15-26)

**Current:**
```rust
pub struct DelegateGeminiParams {
    pub prompt: String,
    pub task_name: Option<String>,
    pub model: Option<String>,
}
```

**Updated:**
```rust
pub struct DelegateGeminiParams {
    pub prompt: String,
    pub task_name: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}
```

**Rationale**: `#[serde(default)]` ensures backward compatibility - missing field deserializes to `None` rather than error.

### 2. handle_delegate_gemini (src/tools/delegate_gemini.rs, around line 66)

**Current pattern** (approximate):
```rust
let gemini = GeminiClient::with_timeout_ms(custom, gemini_timeout_ms());
```

**Updated pattern**:
```rust
// Extract timeout: use provided value, fall back to env var, then DEFAULT
let timeout = params.timeout_ms.unwrap_or_else(gemini_timeout_ms);

// Pass to client
let gemini = GeminiClient::with_timeout_ms(custom, timeout);
```

**For model_override handling** (if applicable):
```rust
// Same timeout extraction before client creation
let timeout = params.timeout_ms.unwrap_or_else(gemini_timeout_ms);

let mut gemini = if let Some(ref model) = params.model {
    GeminiClient::new_with_model(model, timeout)
} else {
    GeminiClient::with_timeout_ms(custom, timeout)
};
```

### 3. delegate_gemini_schema (src/schemas.rs:31-43)

**Add property to properties object**:
```json
"timeout_ms": {
  "type": "number",
  "description": "Optional: Timeout in milliseconds. Overrides GEMINI_TIMEOUT_MS env var if provided."
}
```

**Full context** (if needed):
```json
"properties": {
  "prompt": { "type": "string" },
  "task_name": { "type": "string" },
  "model": { "type": "string" },
  "timeout_ms": {
    "type": "number",
    "description": "Optional: Timeout in milliseconds. Overrides GEMINI_TIMEOUT_MS env var if provided."
  }
}
```

## Fallback Behavior (Priority Order)

1. **Provided parameter** (if `timeout_ms` in call) → use it
2. **Environment variable** (if `GEMINI_TIMEOUT_MS` set) → fall back to it
3. **Default constant** (typically 60s) → fall back to it

This is implemented via:
```rust
let timeout = params.timeout_ms.unwrap_or_else(gemini_timeout_ms);
```

Where `gemini_timeout_ms()` is the existing helper that reads env var or returns DEFAULT.

## Example Usage

### No timeout specified (uses env var or default):
```json
{
  "prompt": "Analyze this",
  "task_name": "quick_analysis"
}
```

### Quick timeout for fast-fail:
```json
{
  "prompt": "Is the system up?",
  "task_name": "health_check",
  "timeout_ms": 5000
}
```

### Extended timeout for heavy lifting:
```json
{
  "prompt": "Full codebase analysis...",
  "task_name": "deep_analysis",
  "timeout_ms": 300000
}
```

## Implementation Checklist

- [ ] Add `timeout_ms: Option<u64>` field to `DelegateGeminiParams` with `#[serde(default)]`
- [ ] Update `handle_delegate_gemini` to extract timeout via `unwrap_or_else(gemini_timeout_ms)`
- [ ] Pass extracted timeout to `GeminiClient::with_timeout_ms()` calls
- [ ] Add `timeout_ms` property to `delegate_gemini_schema` in schemas.rs
- [ ] Verify schema includes proper type and description
- [ ] Build: `cargo build --release`
- [ ] Test compilation: `cargo check`
- [ ] Test clippy: `cargo clippy`
- [ ] Functional test: Call delegate_gemini with timeout_ms param, verify timeout honored
- [ ] Backward compat test: Call without timeout_ms, verify env var/default still works
- [ ] Commit with message: "Add timeout_ms parameter to delegate_gemini for per-call control"

## Design Rationale

**Why Option<u64>?**
- `u64` matches Rust's millisecond representation
- `Option` allows omission without errors
- `#[serde(default)]` handles missing field gracefully

**Why unwrap_or_else?**
- Lazy evaluation: only calls `gemini_timeout_ms()` if param is None
- Clean, idiomatic Rust pattern
- Maintains existing fallback behavior

**Why in schemas.rs?**
- Single source of truth for MCP tool schema
- Ensures descriptor matches implementation
- Auto-validates tool parameter types

## Verification Plan

1. **Compilation**: Should build without warnings
2. **Schema validation**: Tool descriptor should reflect timeout_ms parameter
3. **Backward compatibility**: Existing calls without timeout_ms should work identically
4. **Parameter override**: Call with `timeout_ms: 10000` should use that value, not env var
5. **Fallback chain**: Verify priority order (param > env var > default)

## Notes

- This is a non-breaking change: existing code continues to work
- Parameter is truly optional - callers can omit it entirely
- Enables federation calls with dynamic timeout management
- Complements the existing `cwd` parameter work (separate implementation)
