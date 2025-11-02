# Photography Database Schema

**Database**: SurrealDB @ localhost:8000
**Namespace**: `photography`
**Database**: `ops`

---

## Core Tables

### `skater`
Individual figure skaters who compete at events.

**Fields**:
- `id`: Record ID (`skater:firstname_lastname`)
- `first_name`: String (skater's first name)
- `last_name`: String (skater's last name)
- `created_at`: Datetime (when record was created)

**Example**:
```json
{
  "id": "skater:elayne_lindell",
  "first_name": "Elayne",
  "last_name": "Lindell",
  "created_at": "2025-10-30T12:00:00Z"
}
```

---

### `family`
Family units for gallery delivery. Families are atomic delivery units - all members receive galleries together.

**Fields**:
- `id`: Record ID (`family:lastname` or `family:hyphenated_name`)
- `first_name`: String (required, set to "Family" for most records)
- `last_name`: String (family surname)
- `email`: String (contact email for gallery delivery)
- `created_at`: Datetime (when record was created)

**Example**:
```json
{
  "id": "family:lindell",
  "first_name": "Family",
  "last_name": "Lindell",
  "email": "Mjjoycelindell@gmail.com",
  "created_at": "2025-10-30T12:00:00Z"
}
```

**Notes**:
- `first_name` is required by schema (SCHEMAFULL validation)
- Families can have members with different last names (stepfamilies, etc.)
- Email is optional but required for gallery delivery

---

### `competition`
High-level competition (e.g., "2025 Fall Fling", "2025 Pony Express").

**Fields**:
- `id`: Record ID (`competition:name_identifier`)
- `name`: String (competition display name)
- `date`: String (competition date, format: YYYY-MM-DD)
- `location`: String (venue name)
- `created_at`: Datetime (when record was created)

**Example**:
```json
{
  "id": "competition:2025_fall_fling",
  "name": "2025 Fall Fling",
  "date": "2025-10-25",
  "location": "Line Creek Figure Skating Club",
  "created_at": "2025-10-30T12:00:00Z"
}
```

---

### `event`
Individual skating events within a competition (e.g., "Event 10", "Event 23").

**Fields**:
- `id`: Record ID (`event:competition_id_event_number`)
- `competition_id`: Record pointer to `competition` table
- `event_number`: Integer (event number within competition)
- `created_at`: Datetime (when record was created)

**Example**:
```json
{
  "id": "event:2025_fall_fling_10",
  "competition_id": "competition:2025_fall_fling",
  "event_number": 10,
  "created_at": "2025-10-30T12:00:00Z"
}
```

---

## Relations

### `belongs_to` (skater → family)
Connects individual skaters to their family unit.

**Structure**: `skater →belongs_to→ family`

**Fields**:
- `in`: Record pointer to `skater` table
- `out`: Record pointer to `family` table
- `created_at`: Datetime

**Example**:
```json
{
  "in": "skater:elayne_lindell",
  "out": "family:lindell",
  "created_at": "2025-10-30T12:00:00Z"
}
```

**Business Rule**: Every skater should belong to exactly one family.

---

### `competed_in` (skater → event)
Tracks which events a skater competed in. No status fields - purely tracks participation.

**Structure**: `skater →competed_in→ event`

**Fields**:
- `in`: Record pointer to `skater` table
- `out`: Record pointer to `event` table
- `created_at`: Datetime

**Example**:
```json
{
  "in": "skater:elayne_lindell",
  "out": "event:2025_fall_fling_10",
  "created_at": "2025-10-30T12:00:00Z"
}
```

**Notes**:
- This relation tracks participation only
- Status fields (request_status, gallery_status) belong on `family_competition`
- Old schema incorrectly had status fields here - since removed

---

### `family_competition` (family → competition)
**CRITICAL RELATION**: Tracks gallery request and delivery status at family-competition level.

**Structure**: `family →family_competition→ competition`

**Fields**:
- `in`: Record pointer to `family` table
- `out`: Record pointer to `competition` table
- `request_status`: String (`'requested'` or `'unrequested'`)
- `gallery_status`: String (`'pending'`, `'sent'`, or `'purchased'`)
- `purchase_net`: Float (net amount from purchase, if purchased)
- `created_at`: Datetime

**Example**:
```json
{
  "in": "family:lindell",
  "out": "competition:2025_fall_fling",
  "request_status": "requested",
  "gallery_status": "sent",
  "created_at": "2025-10-30T12:00:00Z"
}
```

**Business Rule**: Families are atomic delivery units. When a family's gallery is sent, ALL members receive it together. Status tracking must happen at this level, NOT at skater-event level.

**Status Values**:

`request_status`:
- `'requested'`: Family explicitly requested gallery (from REQUESTED or VIP sections)
- `'unrequested'`: Family did not request gallery (from UNREQUESTED section)

`gallery_status`:
- `'pending'`: Gallery not yet delivered
- `'sent'`: Gallery sent to family
- `'purchased'`: Family purchased photos (includes `purchase_net` amount)

---

## Schema Enforcement

**SurrealDB Mode**: SCHEMAFULL (strict validation)

**Required Fields**:
- `family.first_name`: Must be provided (cannot be omitted)
- All `id` fields must use proper `type::thing()` syntax
- Record pointers in relations must reference valid records

**Naming Conventions**:
- Table names: lowercase, singular (e.g., `skater`, not `skaters`)
- Record IDs: lowercase with underscores (`elayne_lindell`, not `Elayne-Lindell`)
- Hyphenated names: use underscores in IDs (`laney_sauter` for "Laney-Sauter")

---

## Query Patterns

### Case-Insensitive Family Lookup
```sql
SELECT * FROM family
WHERE string::lowercase(last_name) = string::lowercase('Lindell')
```

### Get Family Status for Competition
```sql
SELECT * FROM family_competition
WHERE in = family:lindell
AND out = competition:2025_fall_fling
```

### Find All Requested Families for Competition
```sql
SELECT in.last_name as family FROM family_competition
WHERE out = competition:2025_fall_fling
AND request_status = 'requested'
```

### Find Pending Galleries
```sql
SELECT in.last_name as family FROM family_competition
WHERE out = competition:2025_fall_fling
AND request_status = 'requested'
AND gallery_status = 'pending'
```

---

## Architecture Decision: Family-Level Status

**Why family_competition instead of competed_in?**

**Problem with competed_in**: Status at skater→event level creates impossible states:
- Sister A: Event 10, status = "sent"
- Sister B: Event 23, status = "pending"
- Reality: Both sisters receive gallery together as family unit

**Solution**: Status tracked at family→competition level enforces business rule:
- Family receives ONE gallery per competition
- All family members included in that gallery
- Status applies to entire family unit
- Impossible states prevented by schema

**Trade-off**: More complex queries, but enforces actual business logic.

---

## Current Schema Issues

**Known Issues**:
- CLI still queries `competed_in` for status (should use `family_competition`)
- Some Python scripts create temporary `/tmp/*.py` files instead of permanent tools
- No validation that every skater has family assignment

**Future Improvements**:
- Add `nickname` field to `skater` for "Tori" → Victoria Han lookups
- Track VIP status on family or skater records
- Add purchase date tracking
- Validate email format on insert
