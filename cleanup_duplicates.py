#!/usr/bin/env python3
"""
Execute duplicate competed_in cleanup based on deletion_plan.json.
"""
import json
from surrealdb import Surreal


def cleanup_duplicates():
    """Delete duplicate competed_in records based on deletion plan."""
    # Load deletion plan
    with open('deletion_plan.json', 'r') as f:
        deletion_plan = json.load(f)

    print(f"Loaded deletion plan: {len(deletion_plan)} records to delete\n")

    # Connect to database
    db = Surreal("ws://localhost:8000/rpc")
    db.signin({"username": "root", "password": "root"})
    db.use("photography", "ops")

    # Get counts before cleanup
    before_count = len(db.query("SELECT * FROM competed_in"))
    print(f"competed_in records BEFORE cleanup: {before_count}")

    # Execute deletions
    print("\nDeleting duplicate records...")
    deleted_count = 0
    failed = []

    for i, item in enumerate(deletion_plan, 1):
        record_id = item['id']
        try:
            db.query(f"DELETE {record_id}")
            deleted_count += 1
            if i % 100 == 0:
                print(f"  Progress: {i}/{len(deletion_plan)} deleted")
        except Exception as e:
            failed.append((record_id, str(e)))
            print(f"  ✗ Failed to delete {record_id}: {e}")

    # Get counts after cleanup
    after_count = len(db.query("SELECT * FROM competed_in"))

    print("\n" + "="*80)
    print("CLEANUP COMPLETE")
    print("="*80)
    print(f"\nRecords BEFORE: {before_count}")
    print(f"Records AFTER: {after_count}")
    print(f"Records DELETED: {deleted_count}")
    print(f"Expected deletion: {len(deletion_plan)}")
    print(f"Failed deletions: {len(failed)}")

    if deleted_count == len(deletion_plan):
        print("\n✅ All planned deletions executed successfully!")
    else:
        print(f"\n⚠️  Some deletions failed ({len(failed)} failures)")
        if failed:
            print("\nFailed records:")
            for record_id, error in failed[:10]:
                print(f"  - {record_id}: {error}")

    # Verify no duplicates remain
    print("\nVerifying cleanup...")
    all_relations = db.query("SELECT * FROM competed_in")
    from collections import defaultdict
    grouped = defaultdict(list)
    for rel in all_relations:
        key = (str(rel['in']), str(rel['out']))
        grouped[key].append(rel)

    remaining_dupes = {k: v for k, v in grouped.items() if len(v) > 1}

    if remaining_dupes:
        print(f"⚠️  WARNING: {len(remaining_dupes)} duplicate pairs still remain")
    else:
        print("✅ No duplicates remain - cleanup successful!")

    db.close()


if __name__ == "__main__":
    cleanup_duplicates()
