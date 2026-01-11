# Phase 2: rethink Tool - Mark Mode

**Status:** Needs Fixes (Testing Failed)
**Parent:** [remini-correction-system.md](../remini-correction-system.md)
**Depends On:** Phase 1 (Schema)
**Assignee:** TBD

---

## Goal

Implement the `rethink` MCP tool with mark creation capability.

---

## Deliverables

- [x] `rethink` tool added to surreal-mind MCP
- [x] `--mark` mode implementation
- [x] Parameter validation
- [x] Database update logic
- [x] Response formatting

**Testing:** See [phase-2-rethink-mark-testing.md](phase-2-rethink-mark-testing.md)

---

## Interface

```bash
rethink <target_id> --mark --type <type> --for <target> --note "..."
```

**Parameters:**
- `target_id`: Record ID (entity:xxx, thought:xxx, observation:xxx)
- `--type`: correction | research | enrich | expand
- `--for`: cc | sam | gemini | dt | gem
- `--note`: Contextual explanation (required)

**Response:**
```json
{
  "success": true,
  "marked": {
    "id": "entity:abc123",
    "type": "correction",
    "for": "gemini",
    "note": "...",
    "marked_at": "2026-01-10T...",
    "marked_by": "cc"
  }
}
```

---

### Implementation Details

**Implementer:** Zed (Grok Code Fast 1)
**Status:** Implemented

- **Schema Definition**: Added `rethink_schema()` function in `src/schemas.rs` defining parameters for `target_id`, `mode` (enum: ["mark"]), `mark_type` (enum: ["correction", "research", "enrich", "expand"]), `marked_for` (enum: ["cc", "sam", "gemini", "dt", "gem"]), and `note` (required string).

- **Tool Handler**: Created `src/tools/rethink.rs` with `RethinkParams` struct and `handle_rethink` implementation that:
  - Validates input parameters against allowed enums
  - Parses target_id to determine table (thoughts, entity/kg_entities, observation/kg_observations)
  - Checks record existence before updating
  - Updates mark fields: `marked_for`, `mark_type`, `mark_note`, `marked_at`, `marked_by`
  - Returns structured JSON response with marked details

- **Integration**: 
  - Added module to `src/tools/mod.rs`
  - Updated `src/server/router.rs` to include tool in `list_tools` and `call_tool` routing
  - Tool name: "rethink", title: "Rethink", description: "Mark records for revision or correction by federation members"

- **Database Logic**:
  - Supports marking thoughts (`thoughts:id`), entities (`entity:id` or `kg_entities:id`), and observations (`observation:id` or `kg_observations:id`)
  - Uses SurrealQL UPDATE to set mark fields with current timestamp
  - `marked_by` currently hardcoded as "cc" (to be made dynamic in future)

- **Validation**:
  - Parameter type validation (mode must be "mark", enums enforced)
  - Target ID format validation (table:id)
  - Record existence check with descriptive error messages
  - Table name normalization (entity → kg_entities, observation → kg_observations)

- **Response Format**: Returns `CallToolResult::structured` JSON with `success: true` and `marked` object containing id, type, for, note, marked_at, marked_by

- **Quality Assurance**: 
  - `cargo fmt` for formatting
  - `cargo clippy` with zero warnings
  - `cargo check` passes
  - `cargo build --release` successful
  - Service restarted via `launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind`
  - Verified running with `curl http://127.0.0.1:8787/health`

- **Change Tracking**: Updated CHANGELOG.md with Phase 2 completion entry

### Known Limitations

- Only "mark" mode implemented; "correct" mode (Phase 4) pending
- `marked_by` hardcoded; should be dynamic based on calling agent/user context
- No integration with wander --mode marks (Phase 3) yet
- No expiration or cleanup logic for marks

---

## Rust Structure

```rust
pub struct RethinkMarkParams {
    pub target_id: String,
    pub mark_type: MarkType,
    pub marked_for: FederationMember,
    pub note: String,
}

pub enum MarkType {
    Correction,
    Research,
    Enrich,
    Expand,
}

pub enum FederationMember {
    CC,
    Sam,
    Gemini,
    DT,
    Gem,
}
```

---

## Review Notes

### Implementation Review (CC - 2026-01-10)

**Verified:**
- Code exists at `src/tools/rethink.rs`
- Registered in router (`src/server/router.rs`)
- Schema defined in `src/schemas.rs`
- Build passes (fmt, clippy, check, release build)
- Service restarts cleanly

**Implementation matches spec:** Yes

**Ready for testing:** Yes - see [testing doc](phase-2-rethink-mark-testing.md)
