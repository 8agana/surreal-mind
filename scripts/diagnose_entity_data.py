#!/usr/bin/env python3
"""
Diagnostic script to examine what fields are stored in kg_entities data
This helps debug chain_id filtering by checking if source_thought_id or staged_by_thought exist
"""

import os
import json
from dotenv import load_dotenv
from surrealdb import Surreal


async def main():
    # Load environment variables
    load_dotenv()

    # Database connection settings
    url = os.getenv("SURR_DB_URL", "ws://127.0.0.1:8000")
    if not url.startswith("ws://") and not url.startswith("wss://"):
        url = f"ws://{url}"
    user = os.getenv("SURR_DB_USER", "root")
    password = os.getenv("SURR_DB_PASS", "root")
    namespace = os.getenv("SURR_DB_NS", "surreal_mind")
    database = os.getenv("SURR_DB_DB", "consciousness")

    print("🔍 Examining entity data structure in SurrealMind database")
    print("=" * 80)

    try:
        async with Surreal(url) as db:
            await db.signin({"username": user, "password": password})
            await db.use(namespace, database)

            print(f"✅ Connected to {namespace}/{database}")
            print()

            # Get sample entities and examine their data
            print("1. Sample entity data inspection:")

            # Get a few entities with their data
            entities = await db.query("""
                SELECT meta::id(id) as id, name, entity_type, data
                FROM kg_entities
                LIMIT 5
            """)

            if entities:
                for i, entity in enumerate(entities):
                    print(
                        f"\nEntity {i + 1}: {entity['name']} ({entity.get('entity_type', 'unknown')})"
                    )
                    print(f"  ID: {entity['id']}")
                    print("  Data fields:")

                    data = entity.get("data", {})
                    if isinstance(data, dict):
                        for key, value in data.items():
                            if key == "description" and len(str(value)) > 100:
                                print(f"    {key}: {str(value)[:100]}...")
                            else:
                                print(f"    {key}: {value}")
                    else:
                        print(f"    (data is not a dict: {type(data)})")
            else:
                print("  No entities found")

            # Check specifically for thought-related fields
            print("\n2. Checking for thought-related fields in entities:")

            # Count entities with different thought reference fields
            counts = await db.query("""
                SELECT
                    count(data.source_thought_id IS NOT NULL) as has_source_thought_id,
                    count(data.staged_by_thought IS NOT NULL) as has_staged_by_thought,
                    count(data.thought_id IS NOT NULL) as has_thought_id,
                    count() as total
                FROM kg_entities
                GROUP ALL
            """)

            if counts:
                count = counts[0]
                print(f"Total entities: {count.get('total', 0)}")
                print(
                    f"Entities with data.source_thought_id: {count.get('has_source_thought_id', 0)}"
                )
                print(
                    f"Entities with data.staged_by_thought: {count.get('has_staged_by_thought', 0)}"
                )
                print(
                    f"Entities with data.thought_id: {count.get('has_thought_id', 0)}"
                )

            # Show examples of each type
            print("\n3. Examples of entities with thought references:")

            # Entities with source_thought_id
            source_examples = await db.query("""
                SELECT meta::id(id) as id, name, data.source_thought_id
                FROM kg_entities
                WHERE data.source_thought_id IS NOT NULL
                LIMIT 3
            """)

            if source_examples:
                print("\nEntities with data.source_thought_id:")
                for entity in source_examples:
                    print(f"  {entity['name']}: {entity['data.source_thought_id']}")

            # Entities with staged_by_thought
            staged_examples = await db.query("""
                SELECT meta::id(id) as id, name, data.staged_by_thought
                FROM kg_entities
                WHERE data.staged_by_thought IS NOT NULL
                LIMIT 3
            """)

            if staged_examples:
                print("\nEntities with data.staged_by_thought:")
                for entity in staged_examples:
                    print(f"  {entity['name']}: {entity['data.staged_by_thought']}")

            # Test the filtering logic manually
            print("\n4. Testing chain_id filtering logic:")

            # Get a sample chain_id
            chain_sample = await db.query("""
                SELECT chain_id FROM thoughts
                WHERE chain_id IS NOT NULL
                LIMIT 1
            """)

            if chain_sample and chain_sample[0]:
                test_chain_id = chain_sample[0]["chain_id"]
                print(f"Testing with chain_id: {test_chain_id}")

                # Test entity filtering with the updated logic
                entity_count = await db.query(f"""
                    SELECT count() as count FROM kg_entities
                    WHERE (data.source_thought_id IN (SELECT meta::id(id) FROM thoughts WHERE chain_id = '{test_chain_id}')
                           OR data.staged_by_thought IN (SELECT meta::id(id) FROM thoughts WHERE chain_id = '{test_chain_id}'))
                    GROUP ALL
                """)

                if entity_count:
                    entities_found = entity_count[0].get("count", 0)
                    print(
                        f"Entities that should be found with this chain_id: {entities_found}"
                    )
                else:
                    print("No entities found with this chain_id")
            else:
                print("No thoughts with chain_id found to test with")

            print("\n" + "=" * 80)

    except Exception as e:
        print(f"❌ Error: {e}")
        import traceback

        traceback.print_exc()


if __name__ == "__main__":
    import asyncio

    asyncio.run(main())
