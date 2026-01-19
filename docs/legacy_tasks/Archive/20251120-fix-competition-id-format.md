# Fix: Competition ID Format in Photography CLI

**Date**: 2025-11-20  
**Discovered by**: CC during duplicate edge cleanup  
**Priority**: HIGH - Breaks DELETE+RELATE pattern

---

## Problem

All photography CLI commands that modify `family_competition` edges are using incorrect competition ID format:

**Current code:**
```rust
let competition_id_only = comp.to_lowercase();  // "pony express"
```

**Should be:**
```rust
let competition_id_only = comp.to_lowercase().replace(" ", "_");  // "pony_express"
```

This causes DELETE queries to fail silently, creating duplicate edges because the DELETE doesn't find the existing edge.

---

## Impact

**Broken commands:**
- `mark-sent` - creates duplicates instead of updating
- `set-status` - creates duplicates instead of updating  
- `request-ty` - creates duplicates instead of updating
- `send-ty` - creates duplicates instead of updating
- `record-purchase` - creates duplicates instead of updating

---

## Fix Required

In `src/photography/commands.rs`, find ALL instances of:
```rust
let competition_id_only = comp.to_lowercase();
```

Replace with:
```rust
let competition_id_only = comp.to_lowercase().replace(" ", "_");
```

**Functions to fix:**
1. `mark_sent()`
2. `set_status()`
3. `request_ty()`
4. `send_ty()`
5. `record_purchase()`

---

## Testing

After fix:
```bash
# This should now properly delete old edge and create new one (no duplicates)
photography mark-sent Williams "Pony Express"
photography check-status pony | grep williams  # Should show ONE entry, not two
```

---

## Database Cleanup

After applying fix, need to manually clean duplicate edges:
```sql
-- Find duplicates
SELECT in.last_name, out.name, count() as edge_count 
FROM family_competition 
GROUP BY in, out 
HAVING edge_count > 1;

-- Delete ALL Pony Express edges (will be recreated correctly)
DELETE family_competition WHERE out.name CONTAINS "Pony";
```

Then re-mark sent families: Mellender, Moritz, Williams, Savoy, Rodriguez
EOF'