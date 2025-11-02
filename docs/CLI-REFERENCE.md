# Photography CLI Reference

**Binary**: `photography` (built from `surreal-mind` Rust project)
**Location**: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/`
**Build Command**: `cargo build --release --bin photography`
**Binary Path**: `target/release/photography`

---

## Running Commands

### Development (with cargo)
```bash
cd ~/Projects/LegacyMind/surreal-mind
cargo run --release --bin photography -- [subcommand] [args]
```

### Production (direct binary)
```bash
cd ~/Projects/LegacyMind/surreal-mind
./target/release/photography [subcommand] [args]
```

---

## Commands

### `query-skater` - Look Up Skater Details

**Usage**: `query-skater <LastName>`

**Description**: Searches for skater by last name and displays all their competition events with request and gallery status.

**Example**:
```bash
cargo run --release --bin photography -- query-skater Lindell
```

**Output**:
```
+-----------+------------+-----------------+---------+----------------+----------------+
| Last Name | First Name | Competition     | Event # | Request Status | Gallery Status |
+-----------+------------+-----------------+---------+----------------+----------------+
| Lindell   | Elayne     | 2025 Fall Fling | 10      | requested      | sent           |
+-----------+------------+-----------------+---------+----------------+----------------+
```

**Search Behavior**:
- Case-insensitive (searches using `string::lowercase()`)
- Returns all skaters with matching last name
- Shows all events they competed in across all competitions
- Displays request and gallery status for each competition

**Note**: Currently pulls status from `competed_in` relation (incorrect - should use `family_competition`).

---

### `get-email` - Get Family Contact Email

**Usage**: `get-email <LastName>`

**Description**: Looks up family email address for gallery delivery.

**Example**:
```bash
cargo run --release --bin photography -- get-email Lindell
```

**Output**:
```
+-----------+--------------------------+
| Last Name | Email                    |
+-----------+--------------------------+
| Lindell   | Mjjoycelindell@gmail.com |
+-----------+--------------------------+
```

**Search Behavior**:
- Case-insensitive (searches using `string::lowercase()`)
- Returns first matching family record
- If multiple families with same last name exist, returns first found

**Use Case**: Quick email lookup when sending galleries.

---

### `pending-galleries` - List Unsent Galleries

**Usage**: `pending-galleries <CompetitionName>`

**Description**: Lists all families with pending (unsent) galleries for a specific competition.

**Example**:
```bash
cargo run --release --bin photography -- pending-galleries "Fall Fling"
```

**Output**:
```
Families with pending galleries for Fall Fling:
- Krug
- Landers
- Laney-Sauter
- McCracken
- Meythaler
- Miller
- Ruiz-Peace
- Sheptor
- Allen
```

**Search Behavior**:
- Partial match on competition name (case-insensitive)
- Filters for `gallery_status = 'pending'`
- Groups by family (not individual skaters)

**Use Case**: See who still needs galleries sent.

---

### `list-events-for-skater` - Show All Events

**Usage**: `list-events-for-skater <LastName>`

**Description**: Lists all competition events a skater participated in, grouped by competition.

**Example**:
```bash
cargo run --release --bin photography -- list-events-for-skater McCracken
```

**Output**:
```
Events for McCracken:

2025 Fall Fling:
- Event 18
- Event 37
- Event 42

2025 Pony Express:
- Event 72
- Event 91
```

**Use Case**: Quick lookup of which events to include in a gallery.

---

### `competition-stats` - Competition Overview

**Usage**: `competition-stats <CompetitionName>`

**Description**: Shows statistics for a competition including total skaters, requested vs unrequested, and delivery status.

**Example**:
```bash
cargo run --release --bin photography -- competition-stats "Fall Fling"
```

**Output**:
```
2025 Fall Fling Statistics:

Total Families: 68
Requested: 30
Unrequested: 38

Gallery Status:
- Pending: 8
- Sent: 19
- Purchased: 3

Purchase Revenue:
Total Net: $96.50
```

**Use Case**: High-level competition tracking and revenue reporting.

---

## CLI Limitations

### Current Issues

1. **Status Source**: CLI queries `competed_in` for status, but status actually lives on `family_competition`. This causes incorrect results in some queries.

2. **No Update Commands**: CLI is read-only. All updates (marking sent, recording purchases) require Python scripts.

3. **Performance Warnings**: Compiler warnings about unused fields (`family_email`, `is_synchro`) appear on every run - these are non-blocking.

4. **Single Family Assumption**: `get-email` assumes one family per last name - breaks with multiple families sharing last name.

---

## Development Notes

### Source Code Location
`/Users/samuelatagana/Projects/LegacyMind/surreal-mind/src/bin/photography.rs`

### Database Connection
```rust
db.connect("ws://localhost:8000/rpc").await?;
db.signin(Root { username: "root", password: "root" }).await?;
db.use_ns("photography").use_db("ops").await?;
```

**Note**: CLI assumes SurrealDB is running on localhost:8000. Won't work on laptop without SSH tunnel to Studio.

### Query Structure Pattern
```rust
let query = format!(
    "SELECT skater.last_name, skater.first_name, competition.name, event.event_number
     FROM competed_in
     WHERE in.last_name CONTAINS '{}'",
    last_name
);
```

### Table Formatting
Uses `prettytable-rs` crate for formatted output.

---

## Future Improvements

**Needed Commands**:
- `mark-sent <Family> <Competition>` - Mark gallery as sent
- `record-purchase <Family> <Competition> <Amount>` - Record purchase
- `import-requests <SourceFile>` - Re-import from markdown
- `verify-data` - Check database matches source files

**Architecture Fixes**:
- Switch all queries to use `family_competition` for status
- Add proper multi-family handling for `get-email`
- Add validation that skaters have family assignments

**User Experience**:
- Add `--json` flag for programmatic output
- Add color coding for status (pending=yellow, sent=green, purchased=blue)
- Add fuzzy search for typos in names
- Add autocomplete for competition names
