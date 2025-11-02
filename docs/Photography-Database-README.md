# Photography Database & CLI System

**Purpose**: Track figure skating competition photography workflow - skaters, families, competitions, gallery requests, and delivery status.

**Technology Stack**:
- **Database**: SurrealDB (localhost:8000, namespace: `photography`, database: `ops`)
- **CLI Tool**: Rust binary (`photography`) built from `surreal-mind` project
- **Source Files**: Markdown files tracking requests and status

---

## Quick Start

### Query a Skater
```bash
cd ~/Projects/LegacyMind/surreal-mind
cargo run --release --bin photography -- query-skater [LastName]
```

### Get Family Email
```bash
cargo run --release --bin photography -- get-email [LastName]
```

### Update Gallery Status (Python)
```python
from surrealdb import Surreal

db = Surreal("ws://localhost:8000/rpc")
db.signin({"username": "root", "password": "root"})
db.use("photography", "ops")

# Mark family as sent
db.query("""
    UPDATE family_competition
    SET gallery_status = 'sent'
    WHERE in = family:lastname
    AND out = competition:2025_fall_fling
""")
```

---

## Documentation Index

### **[SCHEMA.md](SCHEMA.md)** - Database Structure
Complete database schema including:
- All tables (skater, family, competition, event)
- Relations (competed_in, belongs_to, family_competition)
- Field definitions and data types
- Business rules enforced by schema

### **[CLI-REFERENCE.md](CLI-REFERENCE.md)** - Command Line Tool
All CLI commands with examples:
- `query-skater` - Look up skater details
- `get-email` - Get family contact info
- `pending-galleries` - List unsent galleries
- `list-events-for-skater` - Show all events for a skater
- `competition-stats` - Competition overview

### **[MAINTENANCE.md](MAINTENANCE.md)** - Update Operations
Common maintenance tasks:
- Marking galleries as sent
- Recording purchases with net amounts
- Updating request status
- Fixing data quality issues
- Re-importing from source files

### **[SOURCE-FILES.md](SOURCE-FILES.md)** - Source File Formats
Authoritative source file documentation:
- FallFling-SkaterTracking.md structure
- SkaterRequests.md structure
- Section meanings (REQUESTED, VIP, UNREQUESTED)
- How to parse for imports

---

## Architecture Overview

**Data Flow**:
1. Source markdown files contain authoritative request/status data
2. Python import scripts parse markdown → populate SurrealDB
3. Rust CLI queries database for fast lookups
4. Python update scripts modify status based on gallery delivery/purchases

**Key Principle**: Source files are authoritative. Database must match source files.

**Family-Level Status Tracking**: Families are atomic delivery units. Gallery status tracked at `family_competition` level (not individual skaters).

---

## Project Location

**Code**: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/`
- Rust CLI: `src/bin/photography.rs`
- Python scripts: Various `/tmp/*.py` files (temporary, recreate as needed)
- Documentation: `docs/` (this directory)

**Source Files**:
- Fall Fling: `/Volumes/4TB-Sandisk/2025 Fall Fling/Documents/FallFling-SkaterTracking.md`
- Pony Express: `/Volumes/4TB-Sandisk/2025 Pony Express/SkaterRequests.md`

---

## Current State (as of 2025-11-01)

**Fall Fling Requested Families**: 30 total
- Sent: 19
- Purchased: 3 (Whittington $0, Rollie $48.25, Chauhan $48.25)
- Pending: 8

**Database Tables**:
- 240+ skaters
- 240+ families
- 2 competitions (Fall Fling, Pony Express)
- 500+ competed_in relations
- 240+ family_competition relations (status tracking)

---

## For LLMs Working With This System

**Before making changes**:
1. Read SCHEMA.md to understand data structure
2. Check SOURCE-FILES.md to understand authoritative data sources
3. Use CLI-REFERENCE.md for query patterns
4. Follow MAINTENANCE.md for update procedures

**Critical Rules**:
- Source markdown files are always authoritative
- Families are atomic delivery units (status at family level, not skater level)
- Always verify database matches source files before trusting query results
- Case-insensitive searches: wrap in `string::lowercase()`

**When in doubt**: Check source files first, update database to match.
