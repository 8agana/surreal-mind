#!/usr/bin/env python3
"""
Backup photography database to JSON.
"""
import json
from datetime import datetime
from surrealdb import Surreal

def backup_database(output_file: str):
    """Export all tables from photography database to JSON."""
    db = Surreal("ws://localhost:8000/rpc")
    db.signin({"username": "root", "password": "root"})
    db.use("photography", "ops")

    backup_data = {
        "timestamp": datetime.now().isoformat(),
        "namespace": "photography",
        "database": "ops",
        "tables": {}
    }

    # Export all tables
    tables = ["competition", "event", "skater", "family", "competed_in", "belongs_to"]

    for table in tables:
        print(f"Backing up {table}...")
        result = db.query(f"SELECT * FROM {table}")

        # Convert datetime and RecordID objects to strings for JSON serialization
        serializable_result = []
        for record in result:
            serialized = {}
            for key, value in record.items():
                if hasattr(value, 'isoformat'):  # datetime
                    serialized[key] = value.isoformat()
                else:
                    serialized[key] = str(value)
            serializable_result.append(serialized)

        backup_data["tables"][table] = serializable_result
        print(f"  ✓ {len(serializable_result)} records")

    # Write to file
    with open(output_file, 'w') as f:
        json.dump(backup_data, f, indent=2)

    db.close()
    print(f"\n✅ Backup complete: {output_file}")

    # Show stats
    total_records = sum(len(records) for records in backup_data["tables"].values())
    print(f"\nBackup Statistics:")
    for table, records in backup_data["tables"].items():
        print(f"  {table}: {len(records)} records")
    print(f"  TOTAL: {total_records} records")

if __name__ == "__main__":
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    output_file = f"photography_backup_{timestamp}.json"
    backup_database(output_file)
