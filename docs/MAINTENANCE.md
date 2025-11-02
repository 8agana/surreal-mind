# Photography Database Maintenance

Common update operations for managing gallery delivery status, purchases, and data quality.

---

## Python Database Connection

All update operations use Python with the `surrealdb` library:

```python
from surrealdb import Surreal

db = Surreal("ws://localhost:8000/rpc")
db.signin({"username": "root", "password": "root"})
db.use("photography", "ops")

# ... perform updates ...

db.close()
```

---

## Mark Galleries as Sent

### Single Family
```python
from surrealdb import Surreal

db = Surreal("ws://localhost:8000/rpc")
db.signin({"username": "root", "password": "root"})
db.use("photography", "ops")

family_name = 'lindell'
competition_id = '2025_fall_fling'

db.query(f"""
    UPDATE family_competition
    SET gallery_status = 'sent'
    WHERE in IN (
        SELECT VALUE id FROM family WHERE string::lowercase(last_name) = '{family_name}'
    )
    AND out = type::thing('competition', '{competition_id}')
""")

print(f"✅ {family_name.capitalize()}: sent")
db.close()
```

### Multiple Families (Batch Update)
```python
from surrealdb import Surreal

db = Surreal("ws://localhost:8000/rpc")
db.signin({"username": "root", "password": "root"})
db.use("photography", "ops")

sent_families = ['wheeler', 'krug', 'soppe']
competition_id = '2025_fall_fling'

for family_name in sent_families:
    try:
        db.query(f"""
            UPDATE family_competition
            SET gallery_status = 'sent'
            WHERE in IN (
                SELECT VALUE id FROM family WHERE string::lowercase(last_name) = '{family_name}'
            )
            AND out = type::thing('competition', '{competition_id}')
        """)
        print(f"✅ {family_name.capitalize()}: sent")
    except Exception as e:
        print(f"❌ {family_name.capitalize()}: {e}")

db.close()
```

---

## Record Purchases

### Single Purchase
```python
from surrealdb import Surreal

db = Surreal("ws://localhost:8000/rpc")
db.signin({"username": "root", "password": "root"})
db.use("photography", "ops")

family_name = 'rollie'
competition_id = '2025_fall_fling'
net_amount = 48.25

db.query(f"""
    UPDATE family_competition
    SET gallery_status = 'purchased',
        purchase_net = {net_amount}
    WHERE in IN (
        SELECT VALUE id FROM family WHERE string::lowercase(last_name) = '{family_name}'
    )
    AND out = type::thing('competition', '{competition_id}')
""")

print(f"✅ {family_name.capitalize()}: purchased (${net_amount} net)")
db.close()
```

### Multiple Purchases
```python
from surrealdb import Surreal

db = Surreal("ws://localhost:8000/rpc")
db.signin({"username": "root", "password": "root"})
db.use("photography", "ops")

purchases = [
    ('rollie', 48.25),
    ('chauhan', 48.25),
    ('whittington', 0.00)  # Free gallery
]
competition_id = '2025_fall_fling'

for family_name, net_amount in purchases:
    try:
        db.query(f"""
            UPDATE family_competition
            SET gallery_status = 'purchased',
                purchase_net = {net_amount}
            WHERE in IN (
                SELECT VALUE id FROM family WHERE string::lowercase(last_name) = '{family_name}'
            )
            AND out = type::thing('competition', '{competition_id}')
        """)
        print(f"✅ {family_name.capitalize()}: purchased (${net_amount} net)")
    except Exception as e:
        print(f"❌ {family_name.capitalize()}: {e}")

db.close()
```

---

## Fix Request Status (Match Source Files)

When database request_status doesn't match authoritative source files:

```python
from surrealdb import Surreal

db = Surreal("ws://localhost:8000/rpc")
db.signin({"username": "root", "password": "root"})
db.use("photography", "ops")

# All requested families from source file
# (REQUESTED section + VIP section)
requested_families = [
    'ruiz_peace', 'downing', 'bentley', 'soppe', 'elifrits', 'landers',
    'lindell', 'lanham', 'krug', 'anderson', 'rollie', 'chauhan',
    'carrico', 'laney_sauter', 'brown', 'clough', 'vaiciulis', 'davis',
    'hart', 'whittington', 'delaney', 'wheeler', 'mccracken',
    # VIP families
    'allen', 'miller', 'meythaler', 'isaacson', 'butler', 'sheptor', 'durrer'
]

competition_id = '2025_fall_fling'

print("Fixing request_status to match source document...\n")

for family_name in requested_families:
    try:
        db.query(f"""
            UPDATE family_competition
            SET request_status = 'requested'
            WHERE in IN (
                SELECT VALUE id FROM family
                WHERE string::lowercase(string::replace(last_name, '-', '_')) = '{family_name}'
            )
            AND out = type::thing('competition', '{competition_id}')
        """)
        print(f"✅ {family_name}: set to requested")
    except Exception as e:
        print(f"❌ {family_name}: {e}")

db.close()
```

**Note**: Use `string::replace(last_name, '-', '_')` to handle hyphenated names like "Laney-Sauter".

---

## Verify Database Matches Source Files

### Check Requested Families Status

```python
from surrealdb import Surreal

db = Surreal("ws://localhost:8000/rpc")
db.signin({"username": "root", "password": "root"})
db.use("photography", "ops")

# Get all family_competition relations
family_comp_relations = db.query("SELECT * FROM family_competition")
competitions = {str(c['id']): c for c in db.query("SELECT * FROM competition")}
families = {str(f['id']): f for f in db.query("SELECT * FROM family")}

fall_fling_requested_sent = []
fall_fling_requested_pending = []

for rel in family_comp_relations:
    family_id = str(rel['in'])
    comp_id = str(rel['out'])

    comp = competitions.get(comp_id, {})
    comp_name = comp.get('name', '')
    request = rel.get('request_status', '')
    gallery = rel.get('gallery_status', '')

    if 'Fall Fling' in comp_name and request == 'requested':
        family = families.get(family_id, {})
        last_name = family.get('last_name', '?')

        if gallery == 'sent' or gallery == 'purchased':
            fall_fling_requested_sent.append(last_name)
        else:
            fall_fling_requested_pending.append(last_name)

fall_fling_requested_sent.sort()
fall_fling_requested_pending.sort()

print(f"Fall Fling REQUESTED families - SENT: {len(fall_fling_requested_sent)}")
for name in fall_fling_requested_sent:
    print(f"  ✅ {name}")

print(f"\nFall Fling REQUESTED families - PENDING: {len(fall_fling_requested_pending)}")
for name in fall_fling_requested_pending:
    print(f"  ⏳ {name}")

db.close()
```

---

## Query Patterns for Maintenance

### Find Families Without Emails
```python
families_without_emails = db.query("""
    SELECT last_name FROM family
    WHERE email IS NONE OR email = ''
""")

for family in families_without_emails:
    print(f"⚠️  {family['last_name']}: no email")
```

### Find Requested Families with Pending Status
```python
pending_requested = db.query("""
    SELECT in.last_name as family
    FROM family_competition
    WHERE request_status = 'requested'
    AND gallery_status = 'pending'
    AND out = type::thing('competition', '2025_fall_fling')
""")

print(f"Pending requested galleries: {len(pending_requested)}")
for family in pending_requested:
    print(f"  ⏳ {family['family']}")
```

### Calculate Total Revenue
```python
purchases = db.query("""
    SELECT in.last_name as family, purchase_net
    FROM family_competition
    WHERE gallery_status = 'purchased'
    AND out = type::thing('competition', '2025_fall_fling')
""")

total_net = sum(p.get('purchase_net', 0.0) for p in purchases)
print(f"Total net revenue: ${total_net:.2f}")

for purchase in purchases:
    family = purchase.get('family', '?')
    net = purchase.get('purchase_net', 0.0)
    print(f"  {family}: ${net:.2f}")
```

---

## Data Quality Checks

### Check for Skaters Without Family Assignments
```python
# Get all skaters
skaters = {str(s['id']): s for s in db.query("SELECT * FROM skater")}

# Get all belongs_to relations
belongs_to_relations = db.query("SELECT * FROM belongs_to")
skaters_with_families = {str(rel['in']) for rel in belongs_to_relations}

# Find skaters without family
unassigned = []
for skater_id, skater in skaters.items():
    if skater_id not in skaters_with_families:
        last_name = skater.get('last_name', '?')
        first_name = skater.get('first_name', '?')
        unassigned.append(f"{first_name} {last_name}")

if unassigned:
    print(f"⚠️  {len(unassigned)} skaters without family assignments:")
    for name in sorted(unassigned):
        print(f"  {name}")
else:
    print("✅ All skaters have family assignments")
```

### Check for Duplicate competed_in Relations
```python
competed_in_relations = db.query("SELECT * FROM competed_in")

# Group by (skater, event) pair
seen = {}
duplicates = []

for rel in competed_in_relations:
    skater_id = str(rel['in'])
    event_id = str(rel['out'])
    key = (skater_id, event_id)

    if key in seen:
        duplicates.append(key)
    else:
        seen[key] = rel

if duplicates:
    print(f"⚠️  {len(duplicates)} duplicate competed_in relations found")
else:
    print("✅ No duplicate competed_in relations")
```

---

## Re-Import from Source Files

When source files have been updated and database needs to be re-synced:

### Steps:
1. Read authoritative source file (FallFling-SkaterTracking.md or SkaterRequests.md)
2. Parse REQUESTED, VIP, and UNREQUESTED sections
3. Update `family_competition` request_status to match
4. Verify counts match between source and database

**Critical Rule**: Source files are ALWAYS authoritative. When there's a mismatch, source file is correct.

### Example: Re-import Fall Fling Requests
```bash
# 1. Count entries in source file
grep -A 1000 "## REQUESTED" "/Volumes/4TB-Sandisk/2025 Fall Fling/Documents/FallFling-SkaterTracking.md" | grep "^###" | wc -l

# 2. Count VIP entries
grep -A 1000 "## VIP SKATERS" "/Volumes/4TB-Sandisk/2025 Fall Fling/Documents/FallFling-SkaterTracking.md" | grep "^###" | wc -l

# 3. Run Python script to update database (see "Fix Request Status" above)

# 4. Verify database counts match source
```

---

## Common Issues and Solutions

### Issue: "Found NONE for field `first_name`"
**Cause**: Family table requires `first_name` field (SCHEMAFULL validation)
**Solution**: Set `first_name = 'Family'` when creating family records

### Issue: Query returns wrong status
**Cause**: CLI queries `competed_in` for status, but status lives on `family_competition`
**Solution**: Use Python queries directly against `family_competition` table

### Issue: Case-sensitive name matching fails
**Cause**: Database stores names as entered (mixed case)
**Solution**: Use `string::lowercase()` on both sides of comparison

### Issue: Hyphenated name not found
**Cause**: Database uses underscores (`laney_sauter`), query uses hyphens (`laney-sauter`)
**Solution**: Use `string::replace(last_name, '-', '_')` when querying
