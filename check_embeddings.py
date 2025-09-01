#!/usr/bin/env python3
import requests
import json
import math
import base64

# Connect to SurrealDB
url = "http://127.0.0.1:8000/sql"
auth = base64.b64encode(b"root:root").decode()
headers = {
    "Accept": "application/json",
    "Authorization": f"Basic {auth}",
    "NS": "surreal_mind",
    "DB": "consciousness"
}

# Get a few thoughts with embeddings
query = "SELECT meta::id(id) as id, content, embedding FROM thoughts WHERE embedding != NONE LIMIT 5;"
response = requests.post(url, headers=headers, data=query)
result = response.json()

print("=== Embedding Analysis ===\n")
print(f"Response type: {type(result)}")
if isinstance(result, list):
    print(f"Result length: {len(result)}")
    if result:
        print(f"First element keys: {result[0].keys() if isinstance(result[0], dict) else 'Not a dict'}")

if result and isinstance(result, list) and result[0].get('result'):
    for i, thought in enumerate(result[0]['result']):
        if 'embedding' in thought and thought['embedding']:
            emb = thought['embedding']
            content = thought.get('content', '')[:50] + '...'
            
            print(f"Thought {i+1}:")
            print(f"  ID: {thought['id']}")
            print(f"  Content: {content}")
            print(f"  Embedding dims: {len(emb)}")
            
            # Check if all same value (would indicate hash-like behavior)
            unique_vals = len(set(emb))
            print(f"  Unique values: {unique_vals}")
            
            # First 10 values
            print(f"  First 10 values: {[round(x, 4) for x in emb[:10]]}")
            
            # Calculate magnitude
            magnitude = math.sqrt(sum(x*x for x in emb))
            print(f"  Magnitude: {magnitude:.4f}")
            
            # Check if normalized (magnitude should be ~1.0 for normalized)
            is_normalized = abs(magnitude - 1.0) < 0.01
            print(f"  Normalized: {is_normalized}")
            
            # Find min/max
            print(f"  Min/Max: {min(emb):.4f} / {max(emb):.4f}")
            print()

# Now test similarity between two thoughts
print("\n=== Testing Similarity ===\n")
query2 = "SELECT meta::id(id) as id, content, embedding FROM thoughts WHERE content CONTAINS 'Warp' LIMIT 2;"
response2 = requests.post(url, headers=headers, data=query2)
result2 = response2.json()

if result2 and isinstance(result2, list) and result2[0].get('result') and len(result2[0]['result']) >= 2:
    emb1 = result2[0]['result'][0]['embedding']
    emb2 = result2[0]['result'][1]['embedding']
    content1 = result2[0]['result'][0].get('content', '')[:50]
    content2 = result2[0]['result'][1].get('content', '')[:50]
    
    # Calculate cosine similarity
    dot = sum(a*b for a, b in zip(emb1, emb2))
    mag1 = math.sqrt(sum(x*x for x in emb1))
    mag2 = math.sqrt(sum(x*x for x in emb2))
    
    if mag1 > 0 and mag2 > 0:
        similarity = dot / (mag1 * mag2)
        print(f"Content 1: {content1}...")
        print(f"Content 2: {content2}...")
        print(f"Cosine similarity: {similarity:.6f}")
        print(f"Expected: Should be > 0.5 for similar content")