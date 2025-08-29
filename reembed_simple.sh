#!/bin/bash
# Simple re-embedding script using curl

echo "🚀 Starting thought re-embedding process..."
echo "📊 Target dimensions: 768 (from 1536)"

# Load API key
source .env

# Count thoughts
THOUGHT_COUNT=$(echo "SELECT count() FROM thoughts GROUP ALL;" | \
  surreal sql --conn ws://localhost:8000 --user root --pass root \
  --ns surreal_mind --db consciousness 2>/dev/null | \
  grep -o '[0-9]\+' | head -1)

echo "✅ Found $THOUGHT_COUNT thoughts to process"

# Get all thought IDs
echo "SELECT id FROM thoughts;" | \
  surreal sql --conn ws://localhost:8000 --user root --pass root \
  --ns surreal_mind --db consciousness 2>/dev/null | \
  grep -o 'thoughts:[^,}]*' | \
  while read -r THOUGHT_ID; do
    # Get content
    CONTENT=$(echo "SELECT content FROM $THOUGHT_ID;" | \
      surreal sql --conn ws://localhost:8000 --user root --pass root \
      --ns surreal_mind --db consciousness 2>/dev/null | \
      tail -1 | sed 's/.*content: "//' | sed 's/"[,}].*//')
    
    # Skip if empty
    if [ -z "$CONTENT" ]; then
      continue
    fi
    
    echo "Processing: $THOUGHT_ID"
    
    # Get new embedding from OpenAI
    EMBEDDING=$(curl -s https://api.openai.com/v1/embeddings \
      -H "Authorization: Bearer $OPENAI_API_KEY" \
      -H "Content-Type: application/json" \
      -d "{
        \"model\": \"text-embedding-3-small\",
        \"input\": \"$CONTENT\",
        \"dimensions\": 768
      }" | python3 -c "import json,sys; print(json.dumps(json.load(sys.stdin)['data'][0]['embedding']))")
    
    # Update thought with new embedding
    echo "UPDATE $THOUGHT_ID SET embedding = $EMBEDDING;" | \
      surreal sql --conn ws://localhost:8000 --user root --pass root \
      --ns surreal_mind --db consciousness 2>/dev/null
    
    echo "✅ Updated $THOUGHT_ID"
  done

echo "🎯 Re-embedding complete!"