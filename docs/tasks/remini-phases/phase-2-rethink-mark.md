# Phase 2: rethink Tool - Mark Mode

**Status:** Not Started
**Parent:** [remini-correction-system.md](../remini-correction-system.md)
**Depends On:** Phase 1 (Schema)
**Assignee:** TBD

---

## Goal

Implement the `rethink` MCP tool with mark creation capability.

---

## Deliverables

- [ ] `rethink` tool added to surreal-mind MCP
- [ ] `--mark` mode implementation
- [ ] Parameter validation
- [ ] Database update logic
- [ ] Response formatting

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

## Implementation Notes

*To be filled during implementation*

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

## Implementation Notes

*To be filled during implementation*
