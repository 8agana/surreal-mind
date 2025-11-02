#!/usr/bin/env python3
"""
Investigate duplicate competed_in relations in photography database.

Finds relations where the same skater appears multiple times for the same event,
analyzes field differences, and recommends which record to keep.
"""
from collections import defaultdict
from surrealdb import Surreal
from typing import Dict, List


def analyze_duplicates():
    """Find and analyze duplicate competed_in relations."""
    db = Surreal("ws://localhost:8000/rpc")
    db.signin({"username": "root", "password": "root"})
    db.use("photography", "ops")

    print("Loading all competed_in relations...")
    relations = db.query("SELECT * FROM competed_in")

    print(f"Found {len(relations)} total competed_in records\n")

    # Load skater and event data separately for display
    skaters = {str(s['id']): s for s in db.query("SELECT * FROM skater")}
    events = {str(e['id']): e for e in db.query("SELECT * FROM event")}
    competitions = {str(c['id']): c for c in db.query("SELECT * FROM competition")}

    # Group by (skater_id, event_id) to find duplicates
    grouped = defaultdict(list)
    for rel in relations:
        skater_id = str(rel['in'])
        event_id = str(rel['out'])
        key = (skater_id, event_id)
        grouped[key].append(rel)

    # Find duplicates
    duplicates = {k: v for k, v in grouped.items() if len(v) > 1}

    print("="*80)
    print("DUPLICATE ANALYSIS")
    print("="*80)
    print(f"\nTotal unique (skater, event) pairs: {len(grouped)}")
    print(f"Duplicate pairs found: {len(duplicates)}")
    print(f"Total duplicate records: {sum(len(v) - 1 for v in duplicates.values())}")

    if not duplicates:
        print("\n✅ No duplicates found!")
        db.close()
        return

    print("\n" + "="*80)
    print("DETAILED DUPLICATE REPORT")
    print("="*80)

    deletion_plan = []

    for (skater_id, event_id), records in sorted(duplicates.items()):
        # Get human-readable info
        skater = skaters.get(skater_id, {})
        event = events.get(event_id, {})
        comp_id = str(event.get('competition', ''))
        comp = competitions.get(comp_id, {})

        skater_name = f"{skater.get('last_name', 'Unknown')}, {skater.get('first_name', 'Unknown')}"
        event_info = f"{comp.get('name', 'Unknown Competition')} #{event.get('event_number', '?')}"

        print(f"\n📌 DUPLICATE: {skater_name} at {event_info}")
        print(f"   Found {len(records)} records for same (skater, event) pair:")

        # Analyze each record
        record_scores = []
        for i, record in enumerate(records, 1):
            record_id = str(record['id'])

            # Calculate data completeness score
            score = 0
            has_purchase = record.get('purchase_amount') not in [None, 'NONE', '']
            has_notes = record.get('notes') not in [None, 'NONE', '']
            has_gallery_url = record.get('gallery_url') not in [None, 'NONE', '']
            has_purchase_date = record.get('purchase_date') not in [None, 'NONE', '']

            if has_purchase: score += 3  # Purchase data most important
            if has_notes: score += 2
            if has_gallery_url: score += 2
            if has_purchase_date: score += 1

            # Display record details
            print(f"\n   Record {i}: {record_id}")
            print(f"      request_status: {record.get('request_status', 'N/A')}")
            print(f"      gallery_status: {record.get('gallery_status', 'N/A')}")
            print(f"      purchase_amount: {record.get('purchase_amount', 'N/A')}")
            print(f"      purchase_date: {record.get('purchase_date', 'N/A')}")
            print(f"      notes: {record.get('notes', 'N/A')}")
            print(f"      gallery_url: {record.get('gallery_url', 'N/A')}")
            print(f"      created_at: {record.get('created_at', 'N/A')}")
            print(f"      DATA SCORE: {score}/8")

            record_scores.append((i, record_id, score, record))

        # Determine which to keep (highest score, or oldest if tied)
        record_scores.sort(key=lambda x: (-x[2], x[3].get('created_at', '')))
        keep_record = record_scores[0]
        delete_records = record_scores[1:]

        print(f"\n   💡 RECOMMENDATION:")
        print(f"      KEEP: Record {keep_record[0]} (score {keep_record[2]}/8)")
        for rec in delete_records:
            print(f"      DELETE: Record {rec[0]} (score {rec[2]}/8) - ID: {rec[1]}")
            deletion_plan.append({
                'id': rec[1],
                'skater': skater_name,
                'event': event_info,
                'reason': f"Lower data completeness score ({rec[2]}/8 vs {keep_record[2]}/8)"
            })

    # Summary
    print("\n" + "="*80)
    print("DELETION PLAN SUMMARY")
    print("="*80)
    print(f"\nTotal records to delete: {len(deletion_plan)}")
    print(f"Records to keep: {len(duplicates)}")

    if deletion_plan:
        print("\nRecords marked for deletion:")
        for i, item in enumerate(deletion_plan, 1):
            print(f"{i:3d}. {item['id']}")
            print(f"      {item['skater']} at {item['event']}")
            print(f"      Reason: {item['reason']}")

    print("\n" + "="*80)
    print("NEXT STEPS")
    print("="*80)
    print("\n1. Review this report carefully")
    print("2. If deletion plan looks correct, run: python3 cleanup_duplicates.py")
    print("3. Backup already created: photography_backup_20251030_195026.json")

    # Save deletion plan
    import json
    with open('deletion_plan.json', 'w') as f:
        json.dump(deletion_plan, f, indent=2)

    print(f"\n💾 Deletion plan saved to: deletion_plan.json")

    db.close()


if __name__ == "__main__":
    analyze_duplicates()
