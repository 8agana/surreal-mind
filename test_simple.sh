#!/bin/bash

# Simple test script to verify MCP server functionality
# This script tests the basic MCP protocol flow

echo "=== Testing Surreal Mind MCP Server ==="

# Function to send JSON and get response
test_mcp() {
    local input="$1"
    local description="$2"

    echo ""
    echo "--- $description ---"
    echo "Input: $input"
    echo ""

    # Use a temporary file to avoid pipe issues
    echo "$input" | cargo run 2>/dev/null | head -1
}

# Test 1: Initialize only
echo ""
echo "TEST 1: Initialize Protocol"
(
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}'
    sleep 0.2
) | cargo run 2>/dev/null | head -1

echo ""
echo "TEST 2: Full Protocol Flow"
# Create a complete session
{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
    sleep 1
} | timeout 5s cargo run 2>/dev/null || {
    echo "Server may have timed out - this is expected behavior for testing"
}

echo ""
echo "=== Test Complete ==="
echo "If you see initialization responses above, the server is working correctly."
echo "The 'task cancelled' messages in logs are normal when input stream ends."
