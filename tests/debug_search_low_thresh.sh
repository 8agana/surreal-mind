#!/bin/bash

# Debug script for search_thoughts with low threshold and debug logging
echo "=== Debug search_thoughts with Low Threshold ==="

# Enable debug logging
export RUST_LOG=surreal_mind=debug

echo "1. Creating test thought with debug logging..."
{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"convo_think","arguments":{"content":"Artificial intelligence and machine learning are transforming technology","injection_scale":3,"significance":0.8}}}'
    sleep 3
} | SURR_EMBED_PROVIDER=candle cargo run --bin surreal-mind 2>&1 | grep -E 'thought_id|embedding|dimension|error'

echo ""
echo "2. Testing search with VERY low threshold (0.01)..."
{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search_thoughts","arguments":{"content":"artificial intelligence","top_k":5,"sim_thresh":0.01}}}'
    sleep 3
} | SURR_EMBED_PROVIDER=candle cargo run --bin surreal-mind 2>&1 | grep -E 'total|results|similarity|dimension|error'

echo ""
echo "3. Testing search with exact same content..."
{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search_thoughts","arguments":{"content":"Artificial intelligence and machine learning are transforming technology","top_k":5,"sim_thresh":0.01}}}'
    sleep 3
} | SURR_EMBED_PROVIDER=candle cargo run --bin surreal-mind 2>&1 | grep -E 'total|results|similarity|dimension|error'

echo ""
echo "4. Checking database contents..."
# Use the reembed tool to check what's in the database
SURR_EMBED_PROVIDER=candle cargo run --bin reembed -- --dry-run --limit 5 2>/dev/null | grep -E 'processed|missing|mismatched'

echo ""
echo "=== Debug complete ==="
echo "If you still see 'total':0, check:"
echo "  - Embedding dimension warnings in logs"
echo "  - Database connection issues"
echo "  - Thought storage problems"
