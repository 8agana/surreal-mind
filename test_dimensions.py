#!/usr/bin/env python3
"""Test that OpenAI embeddings return the correct dimensions"""

import os
import json
import requests
from dotenv import load_dotenv

load_dotenv()

api_key = os.getenv("OPENAI_API_KEY")
if not api_key:
    print("❌ No OPENAI_API_KEY found")
    exit(1)

# Test different dimension configurations
test_cases = [
    (768, "Testing 768 dimensions"),
    (512, "Testing 512 dimensions"),
    (1536, "Testing default 1536 dimensions"),
]

for dims, description in test_cases:
    print(f"\n🔍 {description}...")
    
    response = requests.post(
        "https://api.openai.com/v1/embeddings",
        headers={"Authorization": f"Bearer {api_key}"},
        json={
            "model": "text-embedding-3-small",
            "input": "Test embedding with custom dimensions",
            "dimensions": dims if dims != 1536 else None
        }
    )
    
    if response.status_code == 200:
        data = response.json()
        embedding = data["data"][0]["embedding"]
        actual_dims = len(embedding)
        
        if actual_dims == dims:
            print(f"✅ Success: Got {actual_dims} dimensions as expected")
        else:
            print(f"❌ Failed: Expected {dims} but got {actual_dims} dimensions")
    else:
        print(f"❌ API Error {response.status_code}: {response.text[:200]}")

print("\n🎯 Dimension test complete!")