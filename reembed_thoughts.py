#!/usr/bin/env python3
"""Re-embed all thoughts with new dimensions (768 instead of 1536)"""

import asyncio
import os
import sys
from surrealdb import Surreal
import requests
import json
from datetime import datetime

# Load environment
OPENAI_API_KEY = os.getenv("OPENAI_API_KEY")
if not OPENAI_API_KEY:
    print("❌ OPENAI_API_KEY not found in environment")
    sys.exit(1)

async def get_embedding(text: str, dims: int = 768) -> list:
    """Get embedding from OpenAI API with specified dimensions"""
    response = requests.post(
        "https://api.openai.com/v1/embeddings",
        headers={"Authorization": f"Bearer {OPENAI_API_KEY}"},
        json={
            "model": "text-embedding-3-small",
            "input": text,
            "dimensions": dims
        }
    )
    
    if response.status_code != 200:
        print(f"❌ API error: {response.status_code} - {response.text[:200]}")
        return None
    
    data = response.json()
    return data["data"][0]["embedding"]

async def main():
    print("🚀 Starting thought re-embedding process...")
    print(f"📊 Target dimensions: 768 (from 1536)")
    
    # Connect to SurrealDB
    db = Surreal("ws://localhost:8000/rpc")
    await db.signin({"user": "root", "pass": "root"})
    await db.use("surreal_mind", "consciousness")
    
    # Get all thoughts
    print("\n📚 Fetching all thoughts from database...")
    thoughts = await db.query("SELECT * FROM thoughts")
    
    if not thoughts or not thoughts[0]['result']:
        print("❌ No thoughts found in database")
        return
    
    thoughts_list = thoughts[0]['result']
    print(f"✅ Found {len(thoughts_list)} thoughts to re-embed")
    
    # Track statistics
    success_count = 0
    error_count = 0
    skip_count = 0
    
    print("\n🔄 Re-embedding thoughts...")
    for i, thought in enumerate(thoughts_list, 1):
        thought_id = thought.get('id')
        content = thought.get('content', '')
        existing_embedding = thought.get('embedding', [])
        
        # Progress indicator
        if i % 10 == 0:
            print(f"  Progress: {i}/{len(thoughts_list)} ({i*100//len(thoughts_list)}%)")
        
        # Skip if already 768 dimensions
        if len(existing_embedding) == 768:
            skip_count += 1
            continue
        
        # Get new embedding
        new_embedding = await get_embedding(content)
        if not new_embedding:
            error_count += 1
            print(f"  ⚠️  Failed to get embedding for thought {thought_id}")
            continue
        
        # Update thought with new embedding
        update_query = f"""
        UPDATE {thought_id} SET 
            embedding = {json.dumps(new_embedding)},
            embedding_model = 'text-embedding-3-small-768',
            embedding_updated_at = '{datetime.utcnow().isoformat()}Z'
        """
        
        try:
            await db.query(update_query)
            success_count += 1
        except Exception as e:
            error_count += 1
            print(f"  ⚠️  Failed to update thought {thought_id}: {e}")
    
    # Final statistics
    print("\n" + "="*50)
    print("📊 RE-EMBEDDING COMPLETE!")
    print(f"✅ Successfully re-embedded: {success_count} thoughts")
    print(f"⏭️  Skipped (already 768-dim): {skip_count} thoughts")
    print(f"❌ Errors: {error_count} thoughts")
    print(f"🎯 New embedding dimensions: 768")
    print("="*50)
    
    # Close connection
    await db.close()

if __name__ == "__main__":
    asyncio.run(main())