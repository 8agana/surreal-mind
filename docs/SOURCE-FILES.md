# Source File Formats

**Critical Principle**: Source markdown files are ALWAYS authoritative. When database and source files disagree, source file is correct.

---

## FallFling-SkaterTracking.md

**Location**: `/Volumes/4TB-Sandisk/2025 Fall Fling/Documents/FallFling-SkaterTracking.md`

**Purpose**: Authoritative record of Fall Fling 2025 competition requests and status.

---

### File Structure

```markdown
# Fall Fling 2025 - Skater Tracking

**Competition Date:** October 25, 2025
**Venue:** Line Creek Figure Skating Club

---

## REQUESTED SKATERS

### [Last Name or Full Name]
**Events:** [comma-separated event numbers]
**Contact:** [Contact person name]
**Email:** [email address]
**Phone:** [phone number]
**Status:** [Not Sent | Sent | Purchased]
**Notes:** [optional notes]

---

## VIP SKATERS

### [Last Name or Full Name]
**Events:** [comma-separated event numbers]
**Status:** [Not Sent | Sent | Purchased]
**Notes:** [why they're VIP]

---

## UNREQUESTED SKATERS

### [Last Name or Full Name]
**Events:** [comma-separated event numbers]
**Status:** Not Requested
**Notes:** [optional notes]
```

---

### Section Meanings

**## REQUESTED SKATERS**
- Families who explicitly signed up for gallery delivery
- These families receive priority editing and delivery
- Should be marked `request_status = 'requested'` in database

**## VIP SKATERS**
- Families who automatically receive galleries without signup
- Includes: Crystal's students, close friends, family connections
- Should be marked `request_status = 'requested'` in database (same as REQUESTED)
- VIP status is indicated by Notes field, not separate database field

**## UNREQUESTED SKATERS**
- Families who competed but did not sign up for galleries
- May still purchase photos if they see them online
- Should be marked `request_status = 'unrequested'` in database

---

### Field Definitions

**Events**: Event numbers the skater competed in at this competition
- Format: Comma-separated integers (e.g., "10, 23, 31")
- Single event: Just the number (e.g., "10")

**Contact**: Name of parent/guardian who requested gallery
- Optional field
- May be omitted if no contact info provided

**Email**: Email address for gallery delivery
- Required for REQUESTED and VIP skaters
- Optional for UNREQUESTED (only populated if they later provide contact info)

**Phone**: Contact phone number
- Optional field
- Format varies (with/without dashes, area code format)

**Status**: Current delivery status
- "Not Sent": Gallery not yet delivered (most common in source file)
- "Sent": Gallery delivered to family
- "Purchased": Family purchased photos
- Note: Source file doesn't track purchase amounts

**Notes**: Free-form notes
- VIP reason ("Crystal's student", "Sam's friend", etc.)
- Special requests
- Follow-up needed

---

### Parsing Rules

**Name Extraction**:
- Section headers (###) contain the name
- May be "First Last" or just "Last Name"
- Database normalizes to `firstname_lastname` format
- Hyphenated names become underscores: "Laney-Sauter" → `laney_sauter`

**Family Grouping**:
- Source file lists individual skaters, not families
- Siblings appear as separate entries with same last name
- Parser must deduplicate by last name to create family records

**Event Parsing**:
- Strip whitespace around commas
- Convert to integers
- Each event becomes separate `competed_in` relation in database

**Email Validation**:
- Must contain "@" to be valid
- Some entries have typos (e.g., "babyjeep486@gmail.co" missing 'm')
- Import as-is, validate separately

---

### Example Entry

```markdown
### Elayne Lindell
**Events:** 10
**Contact:** Mary Joyce-Lindell
**Email:** Mjjoycelindell@gmail.com
**Phone:** 2028708737
**Status:** Not Sent
**Notes:**
```

**Database Result**:
- `skater:elayne_lindell` (first_name='Elayne', last_name='Lindell')
- `family:lindell` (last_name='Lindell', email='Mjjoycelindell@gmail.com')
- `belongs_to` relation: skater:elayne_lindell → family:lindell
- `event:2025_fall_fling_10` (event_number=10)
- `competed_in` relation: skater:elayne_lindell → event:2025_fall_fling_10
- `family_competition` relation: family:lindell → competition:2025_fall_fling (request_status='requested', gallery_status='pending')

---

## SkaterRequests.md

**Location**: `/Volumes/4TB-Sandisk/2025 Pony Express/SkaterRequests.md`

**Purpose**: Authoritative record of Pony Express 2025 competition requests and status.

---

### File Structure

Similar to FallFling-SkaterTracking.md but with slight format variations:

```markdown
# Pony Express 2025 - Skater Requests

## REQUESTED

[Last Name], [First Name]
Events: [comma-separated]
Email: [email address]
Phone: [phone number]
Status: [Requested | Sent | Purchased (Net: $XX.XX)]

## UNREQUESTED

[entries same format as REQUESTED]
```

---

### Format Differences from FallFling-SkaterTracking.md

**Entry Format**:
- Name on single line: "Last Name, First Name" (no ### header)
- Fields prefixed with labels (no **bold** formatting)
- Fewer blank lines between entries

**Status Values**:
- "Requested": Explicitly requested gallery
- "Sent": Gallery delivered
- "Purchased (Net: $XX.XX)": Purchased with net amount shown

**Sections**:
- "## REQUESTED" (not "## REQUESTED SKATERS")
- "## UNREQUESTED" (not "## UNREQUESTED SKATERS")
- No separate VIP section (VIP skaters appear in REQUESTED)

---

### Example Entry

```markdown
Lindell, Elayne
Events: 15, 28
Email: Mjjoycelindell@gmail.com
Phone: 2028708737
Status: Requested
```

**Database Result**: Same as Fall Fling example above, but for Pony Express competition.

---

## Import Workflow

### Full Import Process

1. **Read Source File**
   - Parse competition metadata (date, venue)
   - Identify section boundaries (## REQUESTED, ## VIP, ## UNREQUESTED)

2. **Parse Each Entry**
   - Extract name, events, contact info
   - Normalize name to database format
   - Validate email format

3. **Create/Update Database Records**
   - Create `skater` record if doesn't exist
   - Create `family` record if doesn't exist
   - Create `belongs_to` relation
   - Create `event` records for each event number
   - Create `competed_in` relations
   - Create/update `family_competition` relation with correct request_status

4. **Verify Counts**
   - Count entries in each section of source file
   - Count corresponding records in database
   - Flag mismatches for investigation

---

### Incremental Updates

When source file changes (new requests, status updates):

**Option 1: Re-import Everything**
- Safest approach
- Delete and recreate `family_competition` relations
- Preserves skater/family/event records

**Option 2: Update Only Changed Records**
- Requires tracking what changed
- More efficient but error-prone
- Must verify against source file afterward

**Recommendation**: Re-import for request_status changes, manual updates for gallery_status changes (since status updates happen via database, not source file).

---

## Data Quality Rules

### Critical Rules

1. **Source File is Authoritative for request_status**
   - If source says "REQUESTED", database must say `request_status = 'requested'`
   - If source says "VIP", database must say `request_status = 'requested'`
   - If source says "UNREQUESTED", database must say `request_status = 'unrequested'`

2. **Source File is NOT Authoritative for gallery_status**
   - Gallery delivery happens via ShootProof, updated in database
   - Source file "Status: Sent" may be stale
   - Database gallery_status takes precedence

3. **Source File is NOT Authoritative for purchase_net**
   - Pony Express source shows "Purchased (Net: $XX.XX)" but may be outdated
   - Database purchase_net takes precedence for revenue tracking

### Validation Checks

**After Import, Always Verify**:
```bash
# Count REQUESTED entries in source
grep -c "^###" "/path/to/source.md"  # Fall Fling format
grep -c "^[A-Z]" "/path/to/source.md"  # Pony Express format

# Count requested families in database
# (use Python script from MAINTENANCE.md)

# Counts must match or require investigation
```

---

## Common Parsing Issues

### Issue: Siblings Listed Separately
**Example**:
```markdown
### Macey Boise
**Events:** 20, 42, 46

### Malina Boise
**Events:** 18, 40, 44
```

**Solution**: Parser must create single `family:boise` record with both skaters belonging to it.

### Issue: Hyphenated Names
**Example**: "Hazel Laney-Sauter"
**Database ID**: `skater:hazel_laney_sauter`, `family:laney_sauter`
**Query Requirement**: Use `string::replace(last_name, '-', '_')` for matching

### Issue: Missing Contact Info
**Example**:
```markdown
### Gabriella Chauhan
**Events:** 12
**Status:** Not Sent
**Notes:**
```

**Solution**: Create records with empty email field. Mark as warning for follow-up.

### Issue: Typo in Email
**Example**: "babyjeep486@gmail.co" (missing 'm')
**Solution**: Import as-is, flag for manual correction later.

### Issue: Inconsistent Phone Formatting
**Example**: "816-589-3589" vs "8165893589" vs "(816) 589-3589"
**Solution**: Store as-is. Not used for database queries, only display.

---

## Future Improvements

**Needed Features**:
- Automated import script with section detection
- Diff tool to compare source file vs database
- Validation report highlighting mismatches
- Export from database back to source file format (for sync)

**Nice to Have**:
- Track source file last modified timestamp
- Alert when source file changes
- Automated re-import on source file update
- Version control for source files
