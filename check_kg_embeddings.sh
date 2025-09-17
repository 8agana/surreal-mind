#!/bin/bash

# Check KG embedding status in SurrealDB

echo "=== Checking KG Entities ==="
echo "Total entities:"
echo "SELECT count() FROM kg_entities GROUP ALL;" | surreal sql --conn ws://127.0.0.1:8000 --user root --pass root --ns surreal_mind --db surreal_mind --json | jq -r '.[0].result[0].count'

echo -e "\nEntities WITH embeddings:"
echo "SELECT count() FROM kg_entities WHERE embedding IS NOT NULL GROUP ALL;" | surreal sql --conn ws://127.0.0.1:8000 --user root --pass root --ns surreal_mind --db surreal_mind --json | jq -r '.[0].result[0].count // 0'

echo -e "\nSample entity with embedding (if exists):"
echo "SELECT meta::id(id) as id, name, embedding_dim, array::len(embedding) as actual_dim FROM kg_entities WHERE embedding IS NOT NULL LIMIT 1;" | surreal sql --conn ws://127.0.0.1:8000 --user root --pass root --ns surreal_mind --db surreal_mind --json | jq '.[0].result'

echo -e "\n=== Checking KG Observations ==="
echo "Total observations:"
echo "SELECT count() FROM kg_observations GROUP ALL;" | surreal sql --conn ws://127.0.0.1:8000 --user root --pass root --ns surreal_mind --db surreal_mind --json | jq -r '.[0].result[0].count'

echo -e "\nObservations WITH embeddings:"
echo "SELECT count() FROM kg_observations WHERE embedding IS NOT NULL GROUP ALL;" | surreal sql --conn ws://127.0.0.1:8000 --user root --pass root --ns surreal_mind --db surreal_mind --json | jq -r '.[0].result[0].count // 0'

echo -e "\nSample observation with embedding (if exists):"
echo "SELECT meta::id(id) as id, name, embedding_dim, array::len(embedding) as actual_dim FROM kg_observations WHERE embedding IS NOT NULL LIMIT 1;" | surreal sql --conn ws://127.0.0.1:8000 --user root --pass root --ns surreal_mind --db surreal_mind --json | jq '.[0].result'

echo -e "\n=== Checking Thoughts (for comparison) ==="
echo "Total thoughts:"
echo "SELECT count() FROM thoughts GROUP ALL;" | surreal sql --conn ws://127.0.0.1:8000 --user root --pass root --ns surreal_mind --db surreal_mind --json | jq -r '.[0].result[0].count'

echo -e "\nThoughts WITH embeddings:"
echo "SELECT count() FROM thoughts WHERE embedding IS NOT NULL GROUP ALL;" | surreal sql --conn ws://127.0.0.1:8000 --user root --pass root --ns surreal_mind --db surreal_mind --json | jq -r '.[0].result[0].count'

echo -e "\nSample thought embedding dimensions:"
echo "SELECT meta::id(id) as id, embedding_dim, array::len(embedding) as actual_dim FROM thoughts WHERE embedding IS NOT NULL LIMIT 3;" | surreal sql --conn ws://127.0.0.1:8000 --user root --pass root --ns surreal_mind --db surreal_mind --json | jq '.[0].result'