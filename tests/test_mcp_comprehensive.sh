#!/bin/bash

# Comprehensive MCP Test Script for Surreal Mind Server
# This script tests all major functionality of the MCP server

set -e

echo "=== Comprehensive Surreal Mind MCP Server Test ==="
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counter
TEST_COUNT=0
PASS_COUNT=0

run_test() {
    local test_name="$1"
    local input="$2"
    local expected_pattern="$3"

    TEST_COUNT=$((TEST_COUNT + 1))
    echo -e "${YELLOW}TEST $TEST_COUNT: $test_name${NC}"

    # Run the test and capture both stdout and stderr
    local output
    local exit_code=0

    output=$(echo "$input" | ./target/release/surreal-mind 2>/dev/null) || exit_code=$?

    if [[ $exit_code -eq 0 ]] && echo "$output" | grep -q "$expected_pattern"; then
        echo -e "${GREEN}✅ PASS${NC}"
        echo "Response: $output"
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        echo -e "${RED}❌ FAIL${NC}"
        echo "Expected pattern: $expected_pattern"
        echo "Actual output: $output"
        echo "Exit code: $exit_code"
    fi
    echo ""
}

# Check if binary exists
if [[ ! -f "./target/release/surreal-mind" ]]; then
    echo -e "${RED}Error: Binary not found. Please run 'cargo build --release' first.${NC}"
    exit 1
fi

echo "Testing MCP Protocol Implementation..."
echo ""

# Test 1: Initialize Protocol
run_test "Initialize Protocol" \
'{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' \
'"protocolVersion":"2024-11-05"'

# Test 2: List Tools
run_test "List Tools" \
'{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
'"name":"convo_think"'

# Test 3: Basic convo_think tool call
run_test "Basic convo_think (no injection)" \
'{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"convo_think","arguments":{"content":"This is a test thought for verification.","injection_scale":0}}}' \
'"thought_id"'

# Test 4: convo_think with memory injection
run_test "convo_think with memory injection" \
'{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"convo_think","arguments":{"content":"Another test thought with memory injection enabled.","injection_scale":3}}}' \
'"memories_injected"'

# Test 5: convo_think with high significance
run_test "convo_think with high significance" \
'{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"convo_think","arguments":{"content":"Very important thought that should be highly ranked.","injection_scale":1,"significance":0.9}}}' \
'"enriched_content"'

# Test 6: convo_think with tags and submode
run_test "convo_think with tags and submode" \
'{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"convo_think","arguments":{"content":"Philosophical thought about consciousness.","injection_scale":5,"submode":"philosophical","tags":["philosophy","consciousness","ai"]}}}' \
'"orbital_distances"'

# Test 7: Multiple rapid thoughts (testing memory injection)
run_test "Rapid thought sequence 1" \
'{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"convo_think","arguments":{"content":"First thought in a sequence about machine learning.","injection_scale":2}}}' \
'"thought_id"'

run_test "Rapid thought sequence 2" \
'{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"convo_think","arguments":{"content":"Second thought continuing the machine learning discussion.","injection_scale":3}}}' \
'"memories_injected"'

# Test 8: Edge case - empty content (should fail gracefully)
run_test "Empty content handling" \
'{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"convo_think","arguments":{"content":"","injection_scale":1}}}' \
'"error"\|"thought_id"'

# Test 9: Edge case - invalid injection_scale
run_test "Invalid injection_scale handling" \
'{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"convo_think","arguments":{"content":"Test with invalid scale","injection_scale":10}}}' \
'"error"\|"thought_id"'

# Test 10: Resources list (should be empty)
run_test "List Resources (should be empty)" \
'{"jsonrpc":"2.0","id":11,"method":"resources/list","params":{}}' \
'"resources":\[\]'

echo "=== Test Summary ==="
echo -e "Total Tests: $TEST_COUNT"
echo -e "${GREEN}Passed: $PASS_COUNT${NC}"
echo -e "${RED}Failed: $((TEST_COUNT - PASS_COUNT))${NC}"
echo ""

if [[ $PASS_COUNT -eq $TEST_COUNT ]]; then
    echo -e "${GREEN}🎉 All tests passed! The MCP server is working correctly.${NC}"
    exit 0
else
    echo -e "${YELLOW}⚠️  Some tests failed. Check the output above for details.${NC}"
    exit 1
fi
