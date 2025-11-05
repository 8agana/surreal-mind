#!/usr/bin/env python3
"""
Validate and import ShootProof contacts into photography database.

Purpose: Match CSV contacts to existing DB skaters, populate missing emails,
identify family groupings, and report discrepancies.

Usage:
    ./validate_contacts.py --report  # Show what would change
    ./validate_contacts.py --execute # Actually make changes
"""

import csv
import sys
from collections import defaultdict
from typing import Dict, List, Tuple, Optional
from surrealdb import Surreal
from fuzzywuzzy import fuzz


def parse_csv_name(first_name: str, last_name: str) -> Tuple[List[str], str, bool]:
    """
    Parse CSV name fields into individual first names and shared last name.

    Returns:
        (first_names, last_name, is_family)

    Examples:
        "Jayliana and Jovalee", "Downing" → (["Jayliana", "Jovalee"], "Downing", True)
        "Harper", "Hinton" → (["Harper"], "Hinton", False)
    """
    # Check for family grouping patterns
    if " and " in first_name or "," in first_name:
        # Split on "and" and commas
        parts = first_name.replace(",", " and ").split(" and ")
        first_names = [name.strip() for name in parts if name.strip()]
        return (first_names, last_name, len(first_names) > 1)

    return ([first_name.strip()], last_name.strip(), False)


def fuzzy_match_skater(first_name: str, last_name: str, db_skaters: List[Dict]) -> Optional[Dict]:
    """
    Find best fuzzy match for a skater in the database.

    Returns matching skater dict or None if no good match found.
    """
    best_match = None
    best_score = 0

    for skater in db_skaters:
        # Score based on both first and last name similarity
        first_score = fuzz.ratio(first_name.lower(), skater['first_name'].lower())
        last_score = fuzz.ratio(last_name.lower(), skater['last_name'].lower())

        # Weight last name more heavily (more reliable)
        combined_score = (last_score * 0.6) + (first_score * 0.4)

        if combined_score > best_score and combined_score >= 85:  # 85% threshold
            best_score = combined_score
            best_match = skater

    return best_match


def load_db_data(db: Surreal) -> Tuple[List[Dict], List[Dict]]:
    """Load all skaters and families from database."""
    skaters = db.query("SELECT * FROM skater")
    families = db.query("SELECT * FROM family")

    return skaters if skaters else [], families if families else []


def validate_contacts(csv_path: str, dry_run: bool = True):
    """
    Main validation function.

    Phases:
    1. Load DB data and CSV
    2. Match contacts to skaters
    3. Identify missing emails, family groups, name discrepancies
    4. Report findings
    5. Execute updates if not dry_run
    """
    # Connect to database
    db = Surreal("ws://localhost:8000/rpc")
    db.signin({"username": "root", "password": "root"})
    db.use("photography", "ops")

    # Load current DB state
    print("Loading database state...")
    db_skaters, db_families = load_db_data(db)
    print(f"Found {len(db_skaters)} skaters and {len(db_families)} families in DB\n")

    # Parse CSV
    print(f"Parsing {csv_path}...")
    csv_contacts = []
    with open(csv_path, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            first_names, last_name, is_family = parse_csv_name(
                row['First Name'],
                row['Last Name']
            )
            email = row['Email'].strip() if row['Email'] else None

            csv_contacts.append({
                'first_names': first_names,
                'last_name': last_name,
                'is_family': is_family,
                'email': email,
                'raw_first': row['First Name'],
                'raw_last': row['Last Name']
            })

    print(f"Parsed {len(csv_contacts)} contacts from CSV\n")

    # Match contacts to DB skaters
    print("Matching contacts to database skaters...")
    matches = []
    missing_emails = []
    missing_families = []
    name_mismatches = []
    unmatched_csv = []

    for contact in csv_contacts:
        contact_matches = []

        for first_name in contact['first_names']:
            match = fuzzy_match_skater(first_name, contact['last_name'], db_skaters)
            if match:
                contact_matches.append((first_name, match))

        if contact_matches:
            matches.append({
                'contact': contact,
                'db_matches': contact_matches
            })

            # Check if email is missing
            if contact['email']:
                # Check if any matched skater has family with email
                for first_name, match in contact_matches:
                    # TODO: Check family email once we can query belongs_to
                    missing_emails.append({
                        'skater': match,
                        'email': contact['email'],
                        'first_name': first_name
                    })

            # Check for family grouping
            if contact['is_family'] and len(contact_matches) > 1:
                # TODO: Verify if these skaters are already grouped as family
                missing_families.append({
                    'last_name': contact['last_name'],
                    'first_names': contact['first_names'],
                    'email': contact['email'],
                    'matches': contact_matches
                })
        else:
            unmatched_csv.append(contact)

    # Report findings
    print("\n" + "="*80)
    print("VALIDATION REPORT")
    print("="*80)

    print(f"\n✓ MATCHED: {len(matches)} CSV contacts matched to DB skaters")
    print(f"✗ UNMATCHED: {len(unmatched_csv)} CSV contacts not found in DB (historical data)")

    if missing_emails:
        print(f"\n📧 EMAILS TO POPULATE: {len(missing_emails)}")
        for item in missing_emails[:10]:  # Show first 10
            skater = item['skater']
            print(f"   - {skater['last_name']}, {item['first_name']} → {item['email']}")
        if len(missing_emails) > 10:
            print(f"   ... and {len(missing_emails) - 10} more")

    if missing_families:
        print(f"\n👨‍👩‍👧‍👦 POTENTIAL FAMILY GROUPS: {len(missing_families)}")
        for item in missing_families[:10]:
            names = " and ".join(item['first_names'])
            print(f"   - {item['last_name']}, {names} → {item['email']}")
        if len(missing_families) > 10:
            print(f"   ... and {len(missing_families) - 10} more")

    print("\n" + "="*80)

    if dry_run:
        print("\n🔍 DRY RUN MODE - No changes made")
        print("Run with --execute to apply updates")
    else:
        print("\n⚠️  EXECUTE MODE - Making changes...")

        # Phase 1: Create family groups for multi-skater contacts
        print("\n📥 Creating family groups...")
        families_created = 0
        for item in missing_families:
            family_id = f"family:{item['last_name'].lower().replace(' ', '_')}"

            # Create family record
            db.query(f"""
                INSERT INTO family (id, first_name, last_name, email, created_at)
                VALUES ('{family_id}', 'Family', '{item['last_name']}', '{item['email']}', time::now())
                ON DUPLICATE KEY UPDATE email = '{item['email']}'
            """)

            # Create belongs_to relations for each matched skater
            for first_name, match in item['matches']:
                skater_id = str(match['id']).replace('skater:', '')
                db.query(f"""
                    RELATE (type::thing('skater', '{skater_id}'))->belongs_to->(type::thing('family', '{family_id}'))
                    CONTENT {{ created_at: time::now() }}
                """)

            families_created += 1
            print(f"   ✓ Created family: {item['last_name']} ({len(item['matches'])} siblings)")

        print(f"\n✅ Created {families_created} family groups")

        # Phase 2: Populate emails for individual skaters
        print("\n📧 Populating emails...")
        # Group by skater to avoid duplicates
        skater_emails = {}
        for item in missing_emails:
            skater_id = str(item['skater']['id'])
            if skater_id not in skater_emails:
                skater_emails[skater_id] = {
                    'skater': item['skater'],
                    'email': item['email'],
                    'first_name': item['first_name']
                }

        emails_populated = 0
        for skater_id, data in skater_emails.items():
            skater = data['skater']
            email = data['email']
            last_name = skater['last_name']

            # Check if skater already has a family
            belongs_to_result = db.query(f"""
                SELECT out AS family FROM belongs_to WHERE in = type::thing('skater', '{skater_id.replace("skater:", "")}')
            """)

            if belongs_to_result and len(belongs_to_result) > 0:
                # Update existing family email
                family_id = str(belongs_to_result[0]['family']).replace('family:', '')
                db.query(f"""
                    UPDATE family:{family_id} SET email = '{email}'
                """)
            else:
                # Create new family for this skater
                family_id = f"{last_name.lower().replace(' ', '_')}"
                db.query(f"""
                    INSERT INTO family (id, first_name, last_name, email, created_at)
                    VALUES ('{family_id}', 'Family', '{last_name}', '{email}', time::now())
                    ON DUPLICATE KEY UPDATE email = '{email}'
                """)

                # Create belongs_to relationship
                db.query(f"""
                    RELATE (type::thing('skater', '{skater_id.replace("skater:", "")}'))->belongs_to->(type::thing('family', '{family_id}'))
                    CONTENT {{ created_at: time::now() }}
                """)

            emails_populated += 1

        print(f"✅ Populated {emails_populated} email addresses")
        print("\n🎉 Database updated successfully!")

    db.close()


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: validate_contacts.py [--report|--execute] <csv_path>")
        sys.exit(1)

    mode = sys.argv[1]
    csv_path = sys.argv[2] if len(sys.argv) > 2 else "/Users/samuelatagana/Downloads/contacts-2025-10-30.csv"

    dry_run = mode == "--report"

    validate_contacts(csv_path, dry_run=dry_run)
