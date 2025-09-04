#!/bin/bash

# Test script to verify simplified output format
# This tests that the output simplification is working correctly

set -e
cd "$(dirname "$0")"

echo "=== Testing Simplified Output Format ==="
echo

# Start the server in background and capture PID
timeout 30s ./target/release/surreal-mind > mcp_output.log 2>&1 &
SERVER_PID=$!

# Give server time to start
sleep 2

# Function to send JSON-RPC request
send_request() {
    local request="$1"
    echo "$request" | nc -w 5 localhost 8080 2>/dev/null || {
        # Try direct pipe if nc fails
        echo "$request"
    }
}

# Initialize protocol
INIT_REQUEST='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{"roots":{"listChanged":false}},"clientInfo":{"name":"test","version":"0.1.0"}}}'

# Test think_convo with simplified output
CONVO_REQUEST='{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"think_convo","arguments":{"content":"Test the simplified output format - this should return a clean, concise response","injection_scale":3,"verbose_analysis":false}}}'

# Test think_debug with simplified output
TECH_REQUEST='{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"think_debug","arguments":{"content":"Debug this simplified output implementation","injection_scale":2,"verbose_analysis":false}}}'

echo "Testing via direct binary execution..."

# Create test input file
cat > test_input.json << EOF
$INIT_REQUEST
$CONVO_REQUEST
$TECH_REQUEST
EOF

# Run the test and capture output
timeout 15s ./target/release/surreal-mind < test_input.json > test_output.json 2>test_error.log &
TEST_PID=$!
sleep 10
kill $TEST_PID 2>/dev/null || true

echo "Checking output files..."
if [[ -f test_output.json && -s test_output.json ]]; then
    echo "✓ Generated output file"

    # Look for key patterns in the simplified output
    if grep -q '"thought_id"' test_output.json; then
        echo "✓ Contains thought_id field"
    fi

    if grep -q '"analysis"' test_output.json; then
        echo "✓ Contains analysis block"
    fi

    if grep -q '"key_point"' test_output.json; then
        echo "✓ Contains key_point field"
    fi

    if grep -q '"question"' test_output.json; then
        echo "✓ Contains question field"
    fi

    if grep -q '"next_step"' test_output.json; then
        echo "✓ Contains next_step field"
    fi

    # Check that verbose fields are NOT present
    if ! grep -q '"framework_analysis"' test_output.json; then
        echo "✓ Removed verbose framework_analysis"
    fi

    if ! grep -q '"user_friendly"' test_output.json; then
        echo "✓ Removed verbose user_friendly block"
    fi

    if ! grep -q '"orbital_proximities"' test_output.json; then
        echo "✓ Removed verbose orbital_proximities"
    fi

    # Count lines in output (should be much less than 94)
    OUTPUT_LINES=$(wc -l < test_output.json)
    echo "Output length: $OUTPUT_LINES lines (target: ~10 lines per response)"

    echo
    echo "Sample output:"
    head -20 test_output.json

else
    echo "✗ No output generated"
    echo "Error log:"
    cat test_error.log 2>/dev/null || echo "No error log"
fi

# Clean up
kill $SERVER_PID 2>/dev/null || true
rm -f test_input.json test_output.json test_error.log mcp_output.log

echo
echo "=== Test Complete ==="
