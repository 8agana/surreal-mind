#!/bin/bash

# Simple test script for think_search functionality
echo "=== Testing think_search ==="

# Test 1: Check if tools list includes think_search
echo "1. Checking if think_search tool is available..."
export SURR_EMBED_PROVIDER=candle
{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
    sleep 2
} | cargo run --bin surreal-mind 2>/dev/null | grep -o '"name":"[^"]*"' | grep "think_search"

# Test 2: Simple search test
echo ""
echo "2. Testing think_search with simple query..."
{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"think_search","arguments":{"content":"test search","top_k":3}}}'
    sleep 3
} | cargo run --bin surreal-mind 2>/dev/null | grep -E '"results"|"total"|"error"'

echo ""
echo "=== Test complete ==="
