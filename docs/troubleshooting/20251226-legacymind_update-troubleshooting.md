# legacymind_update troubleshooting 1

**Date**: 2025-12-26
**Issue Type**: Failed Initial Testing
**Status**: Cancelled
**Resolution Date**:
**Original Prompt**: docs/prompts/20251226-legacymind_update-implementation.md

___

## Testing Results

### Test Setup
- **Thought ID**: c0470e78-69aa-48eb-8566-848839d45c61
- **Expected Updates**:
  - `extracted_to_kg` = true
  - `extraction_batch_id` = "ebd00175-9a00-4f5c-b05d-5058028cf9ee"
  - `extracted_at` = (auto-set to now)

### Tool Response
```json
{
  "thought_id": "c0470e78-69aa-48eb-8566-848839d45c61",
  "updated": false,
  "fields_updated": ["extracted_at", "extracted_to_kg", "extraction_batch_id"],
  "reembedded": false
}
```

**Note**: Tool reports `updated: false` despite listing 3 fields in `fields_updated`. This is a contradictory response.

### Database Verification (Post-Update)
Query executed:
```sql
SELECT id, extracted_to_kg, extraction_batch_id FROM thoughts
WHERE id = type::thing('thoughts', 'c0470e78-69aa-48eb-8566-848839d45c61');
```

**Actual Database Values**:
```
extracted_to_kg: false
extraction_batch_id: NONE
id: thoughts:⟨c0470e78-69aa-48eb-8566-848839d45c61⟩
```

**Persistence Result**: ❌ **FAILED** - Fields remain at defaults despite tool response indicating fields were updated.

### Analysis

**The Contradiction**:
- Tool reports `updated: false` but lists 3 fields in `fields_updated`
- This pattern suggests the tool executed the update query but the SurrealDB query itself returned no affected rows
- The tool is correctly reporting what the database told it: "no rows updated"

**Root Cause (Hypothesis)**:
The `legacymind_update` implementation likely uses a SurrealDB UPDATE statement that has a WHERE clause or validation that fails silently:
1. Query executed successfully (no error thrown)
2. WHERE clause matched 0 rows OR update validation rejected the change
3. Tool reports `updated: false` (correct - 0 rows affected)
4. Tool still lists intended fields in `fields_updated` (misleading - they weren't actually updated)

**This mirrors `memories_populate` failure pattern**: Both tools construct update queries that execute without error but don't actually persist changes. The root issue is likely in the SurrealDB query construction, not the Rust code structure.

**Did this succeed where memories_populate failed?**
No. Both tools failed to persist the same fields. The difference is:
- `memories_populate`: Silent failure (reported success, didn't update)
- `legacymind_update`: Explicit failure (reported `updated: false`, didn't update)

`legacymind_update` is more honest about failure, but it's still failing.

**Conclusion from Test 1**:
The tool's honest reporting (`updated: false`) is useful for debugging, but it reveals the fundamental problem: the SurrealDB query being used doesn't match the actual row structure or has validation constraints that prevent updates. Need to:
1. Inspect the actual `legacymind_update.rs` UPDATE query
2. Test query directly against database
3. Verify thought record structure matches what the query expects

---

### Test 2: Fresh Thought Created for Testing

**Test Setup**
- **Date/Time**: 2025-12-26 19:35 CST
- **Thought ID**: 7de0cd87-c823-4e23-9beb-2c61a4ab5638
- **Test Strategy**: Create new thought via `legacymind_think`, immediately update it to eliminate any legacy data issues
- **Goal**: Determine if persistence failure is specific to old thoughts or affects all thoughts

**Test 2a: Update with reembed=false**

Parameters sent:
```json
{
  "thought_id": "7de0cd87-c823-4e23-9beb-2c61a4ab5638",
  "updates": {
    "confidence": 0.85,
    "tags": ["test", "debugging"]
  },
  "reembed": false
}
```

Tool Response:
```json
{
  "thought_id": "7de0cd87-c823-4e23-9beb-2c61a4ab5638",
  "updated": false,
  "fields_updated": ["confidence", "tags"],
  "reembedded": false
}
```

Database Verification (immediately after update):
```sql
SELECT id, confidence, tags FROM thoughts
WHERE id = type::thing('thoughts', '7de0cd87-c823-4e23-9beb-2c61a4ab5638');
```

**Actual Database Values**:
```
confidence: NONE
tags: []
id: thoughts:⟨7de0cd87-c823-4e23-9beb-2c61a4ab5638⟩
```

**Result**: ❌ **FAILED** - No persistence despite explicit reembed=false

---

**Test 2b: Update with reembed=true**

Parameters sent:
```json
{
  "thought_id": "7de0cd87-c823-4e23-9beb-2c61a4ab5638",
  "updates": {
    "confidence": 0.85,
    "tags": ["test", "debugging"]
  },
  "reembed": true
}
```

Tool Response:
```json
{
  "thought_id": "7de0cd87-c823-4e23-9beb-2c61a4ab5638",
  "updated": false,
  "fields_updated": ["confidence", "tags"],
  "reembedded": false
}
```

Database Verification:
```sql
SELECT id, confidence, tags FROM thoughts
WHERE id = type::thing('thoughts', '7de0cd87-c823-4e23-9beb-2c61a4ab5638');
```

**Actual Database Values**:
```
confidence: NONE
tags: []
id: thoughts:⟨7de0cd87-c823-4e23-9beb-2c61a4ab5638⟩
```

**Result**: ❌ **FAILED** - No persistence despite reembed=true

---

### Conclusion from Test 2

**Critical Finding**: Even on a freshly created thought (seconds old), `legacymind_update` fails completely. This eliminates legacy data/migration issues as potential causes.

**What This Tells Us**:
- The UPDATE query construction or execution is fundamentally broken
- Affects ALL thoughts regardless of age (old thoughts from Test 1, fresh thoughts from Test 2)
- Affects ALL field types regardless of target (booleans, strings, arrays, numbers)
- Affects ALL reembed settings (false and true both fail identically)
- The problem is **not about data migration or old record schemas**

**Pattern Observed Across Both Tests**:
1. Tool accepts parameters without validation error
2. Tool reports `fields_updated: [list of fields]` despite `updated: false`
3. Database query after update shows no changes
4. Same failure pattern on old thought (Test 1) and fresh thought (Test 2)

**Indicates**:
The `legacymind_update.rs` implementation has a fundamental issue in how it constructs or executes the SurrealDB UPDATE query. The query either:
- Has a WHERE clause that never matches ANY record
- Has validation constraints that reject ALL updates
- Is using wrong syntax for the thought record structure
- Is targeting a non-existent table or field names

**Next Action Required**:
Inspect `src/tools/legacymind_update.rs` to examine the actual UPDATE query being constructed. The issue is deterministic and reproducible across all scenarios.
