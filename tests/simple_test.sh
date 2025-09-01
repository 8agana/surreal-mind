#!/bin/bash

# Simple test script for search_thoughts functionality
echo "=== Testing search_thoughts ==="

# Test 1: Check if tools list includes search_thoughts
echo "1. Checking if search_thoughts tool is available..."
export SURR_EMBED_PROVIDER=candle
{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
    sleep 2
} | cargo run --bin surreal-mind 2>/dev/null | grep -o '"name":"[^"]*"' | grep "search_thoughts"

# Test 2: Simple search test
echo ""
echo "2. Testing search_thoughts with simple query..."
{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search_thoughts","arguments":{"content":"test search","top_k":3}}}'
    sleep 3
} | cargo run --bin surreal-mind 2>/dev/null | grep -E '"results"|"total"|"error"'

echo ""
echo "=== Test complete ==="
