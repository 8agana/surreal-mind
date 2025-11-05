#!/usr/bin/env python3
"""
Import SkaterRequests.md format into SurrealDB photography database.

Parses markdown structure:
- Lastname Firstname
-    Event X
-    Email: email@example.com
-    Status: Sent | Purchased - $XX.XX (Net: $YY.YY)
-    Note: optional notes

Maps to SurrealDB:
- Skater records
- Event records
- competed_in relations with status
"""

import re
import sys
from datetime import datetime
from typing import Optional
from surrealdb import Surreal

# Status regex patterns
STATUS_PATTERNS = {
    'purchased_full': re.compile(r'Purchased - \$(\d+\.?\d*) \(Net: \$(\d+\.?\d*)\)'),
    'purchased_net': re.compile(r'Purchased \(Net: \$(\d+\.?\d*)\)'),
    'net_only': re.compile(r'\$(\d+\.?\d*) net'),
    'sent': re.compile(r'^Sent$'),
}


def parse_skater_name(name_line: str) -> tuple[str, str]:
    """Parse 'Lastname Firstname' into (first_name, last_name)."""
    parts = name_line.strip().split(None, 1)
    if len(parts) == 2:
        last_name, first_name = parts
        return first_name, last_name
    return parts[0], parts[0]  # Single name edge case


def parse_status(status_line: str) -> tuple[str, Optional[float], Optional[float]]:
    """
    Parse status line into (gallery_status, gross_amount, net_amount).

    Returns:
        - ('sent', None, None) for "Status: Sent"
        - ('purchased', gross, net) for "Status: Purchased - $X (Net: $Y)"
        - ('purchased', None, net) for "Status: Purchased (Net: $X)"
        - ('sent', None, None) as fallback
    """
    status_text = status_line.replace('Status:', '').strip()

    # Try full purchase format
    match = STATUS_PATTERNS['purchased_full'].search(status_text)
    if match:
        return 'purchased', float(match.group(1)), float(match.group(2))

    # Try net-only purchase format
    match = STATUS_PATTERNS['purchased_net'].search(status_text)
    if match:
        return 'purchased', None, float(match.group(1))

    # Try "$XX.XX net" format
    match = STATUS_PATTERNS['net_only'].search(status_text)
    if match:
        return 'purchased', None, float(match.group(1))

    # Default to sent
    return 'sent', None, None


def parse_markdown(filepath: str, section: str = 'Requested') -> list[dict]:
    """
    Parse SkaterRequests.md and extract skater records.

    Args:
        filepath: Path to SkaterRequests.md
        section: Section to parse ('Requested', 'Unrequested', or 'All')

    Returns:
        List of dicts with keys: first_name, last_name, events, email,
                                 gallery_status, gross_amount, net_amount, notes, section
    """
    skaters = []
    current_skater = None
    current_section_name = None

    with open(filepath, 'r') as f:
        for line in f:
            line = line.rstrip()

            # Section detection
            if line.startswith('##'):
                section_name = line.replace('##', '').strip()
                current_section_name = section_name
                continue

            # Skip if not in target section (unless section='All')
            if section != 'All' and current_section_name != section:
                continue

            # Skater name line (starts with capital, not indented)
            if line and not line.startswith(' ') and line[0].isupper():
                # Save previous skater if exists
                if current_skater and current_skater['events']:
                    skaters.append(current_skater)

                # Start new skater
                first_name, last_name = parse_skater_name(line)
                current_skater = {
                    'first_name': first_name,
                    'last_name': last_name,
                    'events': [],
                    'email': None,
                    'gallery_status': 'pending',
                    'gross_amount': None,
                    'net_amount': None,
                    'notes': [],
                    'section': current_section_name
                }

            # Event line
            elif current_skater and line.strip().startswith('Event '):
                event_num = int(line.strip().split()[1])
                current_skater['events'].append(event_num)

            # Email line
            elif current_skater and 'Email:' in line:
                email = line.split('Email:', 1)[1].strip()
                current_skater['email'] = email if email else None

            # Status line
            elif current_skater and 'Status:' in line:
                status, gross, net = parse_status(line)
                current_skater['gallery_status'] = status
                current_skater['gross_amount'] = gross
                current_skater['net_amount'] = net

            # Note line
            elif current_skater and 'Note:' in line:
                note = line.split('Note:', 1)[1].strip()
                current_skater['notes'].append(note)

        # Save last skater
        if current_skater and current_skater['events']:
            skaters.append(current_skater)

    return skaters


def import_to_surrealdb(skaters: list[dict], competition_name: str):
    """
    Import skaters to SurrealDB photography database.

    Uses INSERT...ON DUPLICATE KEY UPDATE pattern from Codex's fix.
    """
    db = Surreal("ws://127.0.0.1:8000")
    db.signin({"username": "root", "password": "root"})
    db.use("photography", "ops")

    # Create competition record (schema: name, venue, start_date, end_date, notes)
    comp_id = competition_name.lower().replace(' ', '_')
    db.query(f"""
        INSERT INTO competition (id, name, venue)
        VALUES ('{comp_id}', '{competition_name}', '')
        ON DUPLICATE KEY UPDATE name = '{competition_name}'
    """)

    total_relations = 0

    for idx, skater in enumerate(skaters, 1):
        print(f"  [{idx}/{len(skaters)}] Processing {skater['first_name']} {skater['last_name']}...")

        # Create skater record ID
        skater_id = f"{skater['last_name']}_{skater['first_name']}".lower().replace('-', '_').replace(' ', '_')

        # Insert skater (schema: first_name, last_name, birth_date, notes, created_at)
        # Note: email goes in family table, not skater table
        try:
            db.query(f"""
                INSERT INTO skater (id, first_name, last_name, created_at)
                VALUES (
                    '{skater_id}',
                    '{skater['first_name']}',
                    '{skater['last_name']}',
                    time::now()
                )
                ON DUPLICATE KEY UPDATE
                    first_name = '{skater['first_name']}',
                    last_name = '{skater['last_name']}'
            """)
        except Exception as e:
            print(f"    ERROR creating skater: {e}")

        # Create competed_in relations for each event
        for event_num in skater['events']:
            event_id = f"{comp_id}_{event_num}"

            # Create event record (matches schema: competition, event_number, split_ice, time_slot)
            # Note: Use NONE for option types, not NULL
            try:
                resp = db.query(f"""
                    INSERT INTO event (id, competition, event_number, split_ice, time_slot)
                    VALUES (
                        '{event_id}',
                        type::thing('competition', '{comp_id}'),
                        {event_num},
                        NONE,
                        NONE
                    )
                    ON DUPLICATE KEY UPDATE event_number = {event_num}
                """)
            except Exception as e:
                print(f"    ERROR creating event {event_num}: {e}")
                continue

            # Create competed_in relation
            # Schema: skate_order, request_status, gallery_status, gallery_url, gallery_sent_at,
            #         purchase_amount, purchase_date, notes, created_at
            # Note: Using net_amount for purchase_amount (what Sam actually received)
            # Map section to request_status: Requested→requested, Unrequested→unrequested
            try:
                purchase_amt = skater['net_amount'] if skater['net_amount'] else 'NONE'
                purchase_date = "time::now()" if skater['gallery_status'] == 'purchased' else 'NONE'

                # Determine request_status from section
                section = skater.get('section', 'Requested')
                if section == 'Unrequested':
                    request_status = 'unrequested'
                else:
                    request_status = 'requested'  # Requested or On-Deck both = requested

                db.query(f"""
                    RELATE (type::thing('skater', '{skater_id}'))->competed_in->(type::thing('event', '{event_id}'))
                    CONTENT {{
                        skate_order: NONE,
                        request_status: '{request_status}',
                        gallery_status: '{skater['gallery_status']}',
                        purchase_amount: {purchase_amt},
                        purchase_date: {purchase_date}
                    }}
                """)
                total_relations += 1
            except Exception as e:
                print(f"    ERROR creating relation for event {event_num}: {e}")

    db.close()
    print(f"✅ Imported {len(skaters)} skaters with {total_relations} event relationships")


def main():
    if len(sys.argv) < 3:
        print("Usage: ./import_skater_requests.py <path_to_SkaterRequests.md> <competition_name> [section]")
        print("Example: ./import_skater_requests.py '/Volumes/4TB-Sandisk/2025 Pony Express/SkaterRequests.md' '2025 Pony Express' All")
        print("Section options: 'Requested', 'Unrequested', 'All' (default: All)")
        sys.exit(1)

    filepath = sys.argv[1]
    competition_name = sys.argv[2]
    section = sys.argv[3] if len(sys.argv) > 3 else 'All'

    print(f"📖 Parsing {filepath} (section: {section})...")
    skaters = parse_markdown(filepath, section=section)
    print(f"   Found {len(skaters)} skaters")

    print(f"\n💾 Importing to SurrealDB (competition: {competition_name})...")
    import_to_surrealdb(skaters, competition_name)

    print("\n✨ Import complete!")


if __name__ == "__main__":
    main()
