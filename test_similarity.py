#!/usr/bin/env python3
import subprocess
import json
import math

def get_embedding(thought_id):
    """Get embedding for a specific thought"""
    cmd = f"""echo "SELECT embedding FROM thoughts WHERE meta::id(id) = '{thought_id}';" | surreal sql --conn http://127.0.0.1:8000 --user root --pass root --ns surreal_mind --db consciousness --json 2>/dev/null"""
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    data = json.loads(result.stdout)
    if data and data[0] and data[0][0]:
        return data[0][0].get('embedding', [])
    return []

def cosine_similarity(a, b):
    """Calculate cosine similarity between two vectors"""
    if len(a) != len(b) or len(a) == 0:
        return 0.0
    
    dot = sum(x*y for x,y in zip(a,b))
    mag_a = math.sqrt(sum(x*x for x in a))
    mag_b = math.sqrt(sum(x*x for x in b))
    
    if mag_a == 0 or mag_b == 0:
        return 0.0
    
    return dot / (mag_a * mag_b)

# Test with the exact thought we searched for earlier
test_thought = "e1eb15a0-d79b-40ec-8c2a-29865c5d1d40"  # The "Warp is back" thought

print("=== Testing Embedding Similarity ===\n")

# Get the embedding
emb1 = get_embedding(test_thought)
if emb1:
    # Check magnitude (should be ~1.0 if normalized)
    magnitude = math.sqrt(sum(x*x for x in emb1))
    print(f"Thought 1 magnitude: {magnitude:.4f} (normalized={abs(magnitude - 1.0) < 0.01})")
    
    # Create a slightly modified query embedding
    # This simulates what should happen with semantic similarity
    import hashlib
    
    # Test 1: Same text should give same embedding (via OpenAI)
    # We'll simulate by using the same embedding
    similarity_same = cosine_similarity(emb1, emb1)
    print(f"\nSimilarity with itself: {similarity_same:.6f} (should be 1.0)")
    
    # Test 2: Get another thought and compare
    # Let's find another thought mentioning Warp
    cmd = """echo "SELECT meta::id(id) as id, content FROM thoughts WHERE content CONTAINS 'test' LIMIT 5;" | surreal sql --conn http://127.0.0.1:8000 --user root --pass root --ns surreal_mind --db consciousness --json 2>/dev/null"""
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    data = json.loads(result.stdout)
    
    if data and data[0]:
        print("\n=== Comparing Different Thoughts ===")
        for thought in data[0][:3]:
            other_id = thought.get('id')
            content = thought.get('content', '')[:60] + '...'
            if other_id and other_id != test_thought:
                emb2 = get_embedding(other_id)
                if emb2:
                    sim = cosine_similarity(emb1, emb2)
                    print(f"\nContent: {content}")
                    print(f"Similarity: {sim:.6f}")
    
    # Test 3: Check if embeddings are actually different
    print("\n=== Embedding Uniqueness Check ===")
    # Get a few more embeddings
    cmd = """echo "SELECT meta::id(id) as id FROM thoughts LIMIT 5;" | surreal sql --conn http://127.0.0.1:8000 --user root --pass root --ns surreal_mind --db consciousness --json 2>/dev/null"""
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    data = json.loads(result.stdout)
    
    if data and data[0]:
        embeddings = []
        for thought in data[0][:3]:
            tid = thought.get('id')
            if tid:
                emb = get_embedding(tid)
                if emb:
                    embeddings.append((tid, emb))
        
        if len(embeddings) >= 2:
            # Compare first 10 values of each
            print("\nFirst 5 values of different embeddings:")
            for tid, emb in embeddings[:2]:
                print(f"{tid}: {[round(x, 4) for x in emb[:5]]}")
            
            # Check if they're identical (which would be bad)
            if len(embeddings) >= 2:
                identical = all(
                    embeddings[0][1][i] == embeddings[1][1][i] 
                    for i in range(min(10, len(embeddings[0][1])))
                )
                print(f"\nEmbeddings identical? {identical} (should be False)")
else:
    print("Could not retrieve embedding for test thought")