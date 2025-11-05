#!/usr/bin/env python3
"""
Diagnostic script to check chain_id and source_thought_id usage in SurrealMind database
"""

import os
import asyncio
import sys
from dotenv import load_dotenv

try:
    from surrealdb import Surreal
except ImportError:
    print("❌ Error: surrealdb library not installed.")
    print("Install with: pip install surrealdb")
    sys.exit(1)


async def main():
    # Load environment variables
    load_dotenv()

    # Database connection settings - use WebSocket
    url = os.getenv("SURR_DB_URL", "ws://127.0.0.1:8000")
    # Ensure WebSocket URL
    if not url.startswith("ws://") and not url.startswith("wss://"):
        url = f"ws://{url}"
    user = os.getenv("SURR_DB_USER", "root")
    password = os.getenv("SURR_DB_PASS", "root")
    namespace = os.getenv("SURR_DB_NS", "surreal_mind")
    database = os.getenv("SURR_DB_DB", "consciousness")

    print(f"🔍 Checking chain_id and source_thought_id usage in SurrealMind database")
    print(f"Connecting to {url}...")
    print("=" * 70)

    try:
        async with Surreal(url) as db:
            await db.signin({"username": user, "password": password})
            await db.use(namespace, database)

            print(f"✅ Connected to {namespace}/{database}")
            print()

            # Check thoughts with chain_id
            print("1. Thoughts with chain_id:")
            result = await db.query(
                "SELECT count() as count FROM thoughts WHERE chain_id IS NOT NULL GROUP ALL"
            )
            thoughts_with_chain_id = result[0]["count"] if result and result[0] else 0
            print(f"   Total thoughts with chain_id: {thoughts_with_chain_id}")

            if thoughts_with_chain_id > 0:
                # Get sample chain_id values and counts
                result = await db.query(
                    "SELECT chain_id, count() as count FROM thoughts WHERE chain_id IS NOT NULL GROUP BY chain_id ORDER BY count DESC LIMIT 10"
                )
                print("   Chain IDs and their usage counts:")
                for row in result:
                    if row:
                        print(f"     {row['chain_id']}: {row['count']} thoughts")

            # Check entities with source_thought_id
            print("\n2. Knowledge Graph Entities with source_thought_id:")
            result = await db.query(
                "SELECT count() as count FROM kg_entities WHERE data.source_thought_id IS NOT NULL GROUP ALL"
            )
            entities_with_source = result[0]["count"] if result and result[0] else 0
            print(f"   Total entities with source_thought_id: {entities_with_source}")

            # Check observations with source_thought_id
            print("\n3. Knowledge Graph Observations with source_thought_id:")
            result = await db.query(
                "SELECT count() as count FROM kg_observations WHERE source_thought_id IS NOT NULL GROUP ALL"
            )
            observations_with_source = result[0]["count"] if result and result[0] else 0
            print(
                f"   Total observations with source_thought_id: {observations_with_source}"
            )

            # Check relationships with source_thought_id
            print("\n4. Knowledge Graph Relationships with source_thought_id:")
            result = await db.query(
                "SELECT count() as count FROM kg_edges WHERE data.source_thought_id IS NOT NULL GROUP ALL"
            )
            edges_with_source = result[0]["count"] if result and result[0] else 0
            print(f"   Total relationships with source_thought_id: {edges_with_source}")

            # Test chain_id filtering
            print("\n5. Testing chain_id filtering (if chain_id exists):")
            if thoughts_with_chain_id > 0:
                result = await db.query(
                    "SELECT chain_id FROM thoughts WHERE chain_id IS NOT NULL LIMIT 1"
                )
                if result and result[0]:
                    test_chain_id = result[0]["chain_id"]
                    print(f"   Testing with chain_id: {test_chain_id}")

                    # Test filtering thoughts by chain_id
                    result = await db.query(
                        "SELECT count() as count FROM thoughts WHERE chain_id = $chain_id GROUP ALL",
                        {"chain_id": test_chain_id},
                    )
                    thoughts_in_chain = (
                        result[0]["count"] if result and result[0] else 0
                    )
                    print(f"   Thoughts with this chain_id: {thoughts_in_chain}")

                    # Test filtering entities by chain_id
                    result = await db.query(
                        "SELECT count() as count FROM kg_entities WHERE data.source_thought_id IN (SELECT meta::id(id) FROM thoughts WHERE chain_id = $chain_id) GROUP ALL",
                        {"chain_id": test_chain_id},
                    )
                    entities_linked = result[0]["count"] if result and result[0] else 0
                    print(
                        f"   Entities linked to thoughts with this chain_id: {entities_linked}"
                    )

                    # Test filtering observations by chain_id
                    result = await db.query(
                        "SELECT count() as count FROM kg_observations WHERE source_thought_id IN (SELECT meta::id(id) FROM thoughts WHERE chain_id = $chain_id) GROUP ALL",
                        {"chain_id": test_chain_id},
                    )
                    observations_linked = (
                        result[0]["count"] if result and result[0] else 0
                    )
                    print(
                        f"   Observations linked to thoughts with this chain_id: {observations_linked}"
                    )

                    # Test filtering relationships by chain_id
                    result = await db.query(
                        "SELECT count() as count FROM kg_edges WHERE data.source_thought_id IN (SELECT meta::id(id) FROM thoughts WHERE chain_id = $chain_id) GROUP ALL",
                        {"chain_id": test_chain_id},
                    )
                    relationships_linked = (
                        result[0]["count"] if result and result[0] else 0
                    )
                    print(
                        f"   Relationships linked to thoughts with this chain_id: {relationships_linked}"
                    )
            else:
                print("   No thoughts with chain_id found to test with")

            print("\n" + "=" * 70)
            print("✅ Diagnostic complete!")

    except Exception as e:
        print(f"❌ Connection or query error: {e}")
        print(
            "Make sure SurrealDB is running and environment variables are set correctly."
        )


if __name__ == "__main__":
    asyncio.run(main())
