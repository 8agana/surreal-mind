#!/bin/bash

# Comprehensive MCP protocol test script for surreal-mind
# Tests initialize, tools/list, and tool call functionality

echo "=== Starting comprehensive MCP test ==="

# Function to send JSON-RPC request and capture response
send_request() {
    local request="$1"
    local description="$2"
    echo "--- $description ---"
    echo "Request: $request"
    echo "$request" | timeout 5s cargo run 2>&1 | tee /tmp/mcp_output.log
    echo ""
}

# Test 1: Initialize handshake
echo "=== Test 1: Initialize Protocol ==="
(
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}'
    sleep 1
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    sleep 1
) | RUST_LOG=debug cargo run 2>&1 | grep -E '"(result|error)"' | head -5

echo ""
echo "=== Test 2: List Tools ==="
# Test 2: List tools after proper initialization
(
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}'
    sleep 0.5
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    sleep 0.5
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
    sleep 2
    echo '{"jsonrpc":"2.0","id":999,"method":"exit"}'
) | RUST_LOG=debug cargo run 2>&1 | grep -A 10 -B 5 '"tools"'

echo ""
echo "=== Test 3: Call Tool ==="
# Test 3: Call the think_convo tool
(
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}'
    sleep 0.5
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    sleep 0.5
    echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"think","arguments":{"content":"This is a test thought for MCP validation"}}}'
    sleep 3
    echo '{"jsonrpc":"2.0","id":999,"method":"exit"}'
) | RUST_LOG=debug cargo run 2>&1 | grep -A 20 -B 5 '"result"'

echo ""
echo "=== Test 4: Raw Protocol Flow ==="
# Test 4: Show full protocol flow
(
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}'
    sleep 1
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    sleep 1
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
    sleep 2
) | RUST_LOG=rmcp=trace,surreal_mind=debug cargo run 2>&1 | grep -E '(jsonrpc|tools|result|error)' | jq -s '.'

echo ""
echo "=== Test Complete ==="
