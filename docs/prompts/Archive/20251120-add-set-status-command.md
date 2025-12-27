# Photography CLI: Add set-status Command

**Date**: 2025-11-20  
**Requested by**: Sam  
**Context**: CC identified missing functionality, Sam approved implementation via Grok

---

## Problem

The photography database schema supports 7 gallery status values:
- `pending`, `culling`, `processing`, `sent`, `purchased`, `not_shot`, `needs_research`

But the CLI only has commands to set 2 of them:
- `mark-sent` → sets to `sent`
- `record-purchase` → sets to `purchased`

**Missing**: No way to set `needs_research`, `not_shot`, or the workflow states (`culling`, `processing`).

**Immediate need**: Sam has 8 families that need to be marked `needs_research` (back-burner cases requiring investigation before delivery).

---

## Goal

Add `set-status` command to photography CLI:

```bash
photography set-status <family_last_name> <competition_name> <status>
```

Example usage:
```bash
photography set-status Bair "Pony Express" needs_research
photography set-status Williams "Pony Express" culling
```

---

## Implementation Requirements

### 1. **Command Function** (`src/photography/commands.rs`)

Add `set_status()` function following the established pattern:

```rust
pub async fn set_status(
    db: &Surreal<Client>,
    last_name: &str,
    comp: &str,
    status: &str,
) -> Result<()>
```

**Key requirements**:
- **Validation**: Status must be one of: `pending`, `culling`, `processing`, `sent`, `purchased`, `not_shot`, `needs_research`
- **DELETE+RELATE pattern**: Use same pattern as `mark_sent`, `request_ty`, `send_ty`, `record_purchase` to prevent duplicate edges
- **Family existence check**: Verify family exists before attempting status change
- **Clear output**: Print confirmation with family name and new status

**Pattern to follow** (from `mark_sent`):
```rust
// Check family exists
let check_sql = r#"SELECT * FROM type::thing('family', $id)"#;
let mut check_resp = db.query(check_sql).bind(("id", family_id_only.clone())).await?;
let check: Vec<Family> = check_resp.take(0)?;
if check.is_empty() {
    println!("❌ Error: Family {} not found.", family_id_full);
    return Ok(());
}

// DELETE+RELATE for clean UPSERT
let delete_sql = "
    DELETE family_competition
    WHERE in = type::thing('family', $family_id)
    AND out = type::thing('competition', $competition_id)
";
let _ = db.query(delete_sql)
    .bind(("family_id", family_id_only.clone()))
    .bind(("competition_id", competition_id_only.clone()))
    .await?;

let sql = "
    RELATE (type::thing('family', $family_id))->family_competition->(type::thing('competition', $competition_id))
    SET gallery_status = $status
";
let _ = db.query(sql)
    .bind(("family_id", family_id_only))
    .bind(("competition_id", competition_id_only))
    .bind(("status", status))
    .await?;
```

### 2. **CLI Integration** (`src/bin/photography.rs`)

Add subcommand to the CLI parser:

```rust
SetStatus {
    #[arg(help = "Family last name")]
    last_name: String,
    #[arg(help = "Competition name")]
    competition: String,
    #[arg(help = "Status: pending|culling|processing|sent|purchased|not_shot|needs_research")]
    status: String,
},
```

Add match arm in command handler:
```rust
Commands::SetStatus { last_name, competition, status } => {
    commands::set_status(&db, &last_name, &competition, &status).await?;
}
```

### 3. **Testing**

After implementation, test with:
```bash
# Mark families as needs_research
photography set-status Bair "Pony Express" needs_research
photography set-status Ketcherside "Pony Express" needs_research

# Verify with check-status
photography check-status pony

# Test validation (should fail gracefully)
photography set-status Bair "Pony Express" invalid_status
```

### 4. **Documentation** (`CHANGELOG.md`)

Add entry at top:
```markdown
## 2025-11-20 - Photography CLI: Add set-status Command
- **Added set-status command**: Generic status setter for all gallery_status values
  - Usage: `photography set-status <family> <competition> <status>`
  - Validates status against schema-defined values: pending, culling, processing, sent, purchased, not_shot, needs_research
  - Uses DELETE+RELATE pattern for clean edge creation/updates
  - Enables workflow triage: mark families as needs_research (investigation required) or not_shot (split ice, no coverage)
- **Immediate use case**: 8 Pony Express families marked needs_research for back-burner handling
```

---

## Files to Modify

1. **`src/photography/commands.rs`**: Add `set_status()` function (~40 lines following existing pattern)
2. **`src/bin/photography.rs`**: Add `SetStatus` subcommand and match arm (~10 lines)
3. **`CHANGELOG.md`**: Document the new command (~5 lines)

---

## Success Criteria

- [ ] Command compiles without errors/warnings
- [ ] Can set any of the 7 valid statuses
- [ ] Rejects invalid statuses with clear error message
- [ ] Uses DELETE+RELATE pattern (no duplicate edges)
- [ ] check-status shows updated status correctly
- [ ] CHANGELOG updated

---

## Notes

- **Why not individual commands**: One flexible command is simpler than 5 separate commands (mark-needs-research, mark-not-shot, etc.)
- **Consistency**: This follows the same DELETE+RELATE pattern we just debugged and fixed in Session 3
- **Sam's immediate need**: After implementation, he'll mark 8 families as needs_research to clear them from the working pending list
