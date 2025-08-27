#!/usr/bin/env python3
"""
Re-embed all thoughts in SurrealDB with OpenAI text-embedding-3-small
"""

import os
import asyncio
from typing import List, Dict, Any
import httpx
from surreal import Surreal
from dotenv import load_dotenv
import json

# Load environment variables
load_dotenv()

OPENAI_API_KEY = os.getenv("OPENAI_API_KEY")
if not OPENAI_API_KEY:
    raise ValueError("OPENAI_API_KEY not found in environment")

async def get_embedding(text: str) -> List[float]:
    """Get embedding from OpenAI API"""
    async with httpx.AsyncClient() as client:
        response = await client.post(
            "https://api.openai.com/v1/embeddings",
            headers={
                "Authorization": f"Bearer {OPENAI_API_KEY}",
                "Content-Type": "application/json",
            },
            json={
                "model": "text-embedding-3-small",
                "input": text,
            },
            timeout=30.0
        )
        response.raise_for_status()
        data = response.json()
        return data["data"][0]["embedding"]

async def main():
    """Main re-embedding function"""
    print("🚀 Starting re-embedding process...")
    
    # Connect to SurrealDB
    async with Surreal("ws://127.0.0.1:8000") as db:
        await db.signin({"username": "root", "password": "root"})
        await db.use("surreal_mind", "consciousness")
        
        # Get all thoughts
        result = await db.query("SELECT id, content FROM thoughts")
        thoughts = result[0]["result"] if result else []
        
        print(f"📊 Found {len(thoughts)} thoughts to re-embed")
        
        # Re-embed each thought
        success = 0
        failed = 0
        
        for i, thought in enumerate(thoughts):
            try:
                thought_id = thought["id"]
                content = thought["content"]
                
                print(f"🔄 [{i+1}/{len(thoughts)}] Re-embedding thought {thought_id}...")
                
                # Get new embedding
                embedding = await get_embedding(content)
                
                # Update thought with new embedding
                update_query = f"""
                UPDATE {thought_id} SET 
                    embedding = {json.dumps(embedding)},
                    embedding_model = 'text-embedding-3-small',
                    embedding_dim = 1536
                """
                
                await db.query(update_query)
                success += 1
                
            except Exception as e:
                print(f"❌ Failed to re-embed {thought.get('id', 'unknown')}: {e}")
                failed += 1
                
        print(f"\n✅ Re-embedding complete!")
        print(f"   Success: {success}")
        print(f"   Failed: {failed}")

if __name__ == "__main__":
    asyncio.run(main())