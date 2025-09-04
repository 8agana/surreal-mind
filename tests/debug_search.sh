#!/bin/bash

# Simple debug script for think_search functionality
echo "=== Debugging think_search ==="

# First, let's test if the server starts properly
echo "1. Testing server startup..."
export SURR_EMBED_PROVIDER=candle
timeout 5s cargo run --bin surreal-mind -- --help 2>&1 | head -5

echo ""
echo "2. Testing basic MCP protocol..."
{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
    sleep 1
} | SURR_EMBED_PROVIDER=candle timeout 5s cargo run --bin surreal-mind 2>&1 | head -10

echo ""
echo "3. Testing tools list..."
{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
    sleep 2
} | SURR_EMBED_PROVIDER=candle timeout 10s cargo run --bin surreal-mind 2>&1 | grep -E '\"name\"|\"error\"' | head -5

echo ""
echo "4. Testing think_search directly..."
{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"think_search","arguments":{"content":"test search","top_k":3}}}'
    sleep 3
} | SURR_EMBED_PROVIDER=candle timeout 10s cargo run --bin surreal-mind 2>&1 | head -20

echo ""
echo "=== Debug complete ==="
