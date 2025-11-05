#!/usr/bin/env python3
"""
Test script to create sample data with chain_id and test chain_id filtering
"""

import asyncio
import os
from datetime import datetime
from dotenv import load_dotenv

try:
    from surrealdb import Surreal
except ImportError:
    print("❌ Error: surrealdb library not installed.")
    print("Install with: pip install surrealdb")
    exit(1)


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

    print("🧪 Testing chain_id functionality in SurrealMind database")
    print(f"Connecting to {url}...")
    print("=" * 70)

    try:
        async with Surreal(url) as db:
            await db.signin({"username": user, "password": password})
            await db.use(namespace, database)

            print(f"✅ Connected to {namespace}/{database}")

            # Create test chain_id
            test_chain_id = f"test-chain-{int(datetime.now().timestamp())}"
            print(f"\n🔗 Using test chain_id: {test_chain_id}")

            # Create test thoughts with chain_id
            print("\n1. Creating test thoughts with chain_id...")

            thought1 = {
                "content": "This is a test thought in the chain",
                "significance": 0.8,
                "chain_id": test_chain_id,
                "created_at": datetime.now().isoformat(),
                "embedding": [0.1] * 1536,  # Dummy embedding
                "embedding_dim": 1536,
                "embedding_provider": "openai",
                "embedding_model": "text-embedding-3-small",
            }

            thought2 = {
                "content": "This is another test thought in the same chain",
                "significance": 0.7,
                "chain_id": test_chain_id,
                "created_at": datetime.now().isoformat(),
                "embedding": [0.2] * 1536,  # Dummy embedding
                "embedding_dim": 1536,
                "embedding_provider": "openai",
                "embedding_model": "text-embedding-3-small",
            }

            # Insert thoughts
            result1 = await db.create("thoughts", thought1)
            thought1_id = result1[0]["id"]
            print(f"   Created thought 1: {thought1_id}")

            result2 = await db.create("thoughts", thought2)
            thought2_id = result2[0]["id"]
            print(f"   Created thought 2: {thought2_id}")

            # Create test memories linked to these thoughts
            print("\n2. Creating test memories linked to thoughts...")

            # Entity
            entity = {
                "name": "TestEntity",
                "entity_type": "concept",
                "data": {
                    "source_thought_id": thought1_id,
                    "description": "A test concept entity",
                },
                "created_at": datetime.now().isoformat(),
                "confidence": 0.9,
            }

            # Observation
            observation = {
                "name": "TestObservation",
                "data": {
                    "source_thought_id": thought2_id,
                    "description": "A test observation",
                },
                "created_at": datetime.now().isoformat(),
            }

            # Insert memories
            entity_result = await db.create("kg_entities", entity)
            entity_id = entity_result[0]["id"]
            print(f"   Created entity: {entity_id}")

            obs_result = await db.create("kg_observations", observation)
            obs_id = obs_result[0]["id"]
            print(f"   Created observation: {obs_id}")

            # Test the chain_id filtering
            print("\n3. Testing chain_id filtering...")

            # Test thoughts filtering
            thoughts_query = await db.query(
                "SELECT meta::id(id) as id, content FROM thoughts WHERE chain_id = $chain_id",
                {"chain_id": test_chain_id},
            )
            thoughts_found = len(thoughts_query) if thoughts_query else 0
            print(
                f"   Thoughts found with chain_id '{test_chain_id}': {thoughts_found}"
            )

            # Test entities filtering (using the same logic as unified search)
            entities_query = await db.query(
                "SELECT meta::id(id) as id, name FROM kg_entities WHERE data.source_thought_id IN (SELECT meta::id(id) FROM thoughts WHERE chain_id = $chain_id)",
                {"chain_id": test_chain_id},
            )
            entities_found = len(entities_query) if entities_query else 0
            print(f"   Entities linked to chain '{test_chain_id}': {entities_found}")

            # Test observations filtering
            obs_query = await db.query(
                "SELECT meta::id(id) as id, name FROM kg_observations WHERE source_thought_id IN (SELECT meta::id(id) FROM thoughts WHERE chain_id = $chain_id)",
                {"chain_id": test_chain_id},
            )
            obs_found = len(obs_query) if obs_query else 0
            print(f"   Observations linked to chain '{test_chain_id}': {obs_found}")

            # Summary
            print("\n📊 Test Results:")
            print(f"   Thoughts created: 2")
            print(f"   Entities created: 1")
            print(f"   Observations created: 1")
            print(f"   Thoughts found via chain_id: {thoughts_found}")
            print(f"   Entities found via chain_id: {entities_found}")
            print(f"   Observations found via chain_id: {obs_found}")

            if thoughts_found == 2 and entities_found >= 1 and obs_found >= 1:
                print("\n✅ SUCCESS: chain_id filtering is working correctly!")
                print(f"   You can now test the search with chain_id: {test_chain_id}")
            else:
                print("\n❌ FAILURE: chain_id filtering is not working")
                print("   Check the unified_search.rs implementation")

            print("\n" + "=" * 70)

    except Exception as e:
        print(f"❌ Error: {e}")
        import traceback

        traceback.print_exc()


if __name__ == "__main__":
    asyncio.run(main())
