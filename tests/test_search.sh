#!/bin/bash

# Test script for search_thoughts functionality
# This script tests the search_thoughts tool with various parameters

set -e

echo "=== Testing search_thoughts Tool ==="
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counter
TEST_COUNT=0
PASS_COUNT=0

run_search_test() {
    local test_name="$1"
    local search_query="$2"
    local expected_pattern="$3"
    local additional_params="$4"

    TEST_COUNT=$((TEST_COUNT + 1))
    echo -e "${YELLOW}TEST $TEST_COUNT: $test_name${NC}"
    echo "Query: '$search_query'"

    # Create the search request
    local search_request=$(cat <<EOF
{
    "jsonrpc": "2.0",
    "id": $TEST_COUNT,
    "method": "tools/call",
    "params": {
        "name": "search_thoughts",
        "arguments": {
            "content": "$search_query",
            $additional_params
            "top_k": 5
        }
    }
}
EOF
    )

    # Send the request and capture output
    local output
    local exit_code=0

    # Use a temporary file for the request
    echo "$search_request" > /tmp/search_request.json

output=$(cat /tmp/search_request.json | SURR_EMBED_PROVIDER=candle timeout 10s cargo run --bin surreal-mind 2>/dev/null) || exit_code=$?

    if [[ $exit_code -eq 0 ]] && echo "$output" | grep -q "$expected_pattern"; then
        echo -e "${GREEN}✅ PASS${NC}"
        echo "Response contains expected pattern: $expected_pattern"
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        echo -e "${RED}❌ FAIL${NC}"
        echo "Expected pattern: $expected_pattern"
        echo "Exit code: $exit_code"
        echo "Output: $output"
    fi
    echo ""
}

# Check if binary exists
if [[ ! -f "./target/debug/surreal-mind" ]]; then
    echo -e "${YELLOW}Building debug binary...${NC}"
    cargo build
fi

echo "Testing search_thoughts functionality..."
echo ""

# Test 1: Basic search
run_search_test "Basic semantic search" \
    "test thought" \
    '"total"' \
    ""

# Test 2: Search with lower similarity threshold
run_search_test "Search with low threshold" \
    "consciousness" \
    '"total"' \
    '"sim_thresh": 0.3,'

# Test 3: Search with specific submode
run_search_test "Search with submode filter" \
    "technical discussion" \
    '"total"' \
    '"submode": "technical",'

# Test 4: Search with date range (empty if no data)
run_search_test "Search with date range" \
    "memory" \
    '"total"' \
    '"date_range": {"from": "2024-01-01T00:00:00Z", "to": "2024-12-31T23:59:59Z"},'

# Test 5: Search with graph expansion
run_search_test "Search with graph expansion" \
    "related thoughts" \
    '"total"' \
    '"expand_graph": true, "graph_depth": 1,'

echo "=== Search Test Summary ==="
echo -e "Total Tests: $TEST_COUNT"
echo -e "${GREEN}Passed: $PASS_COUNT${NC}"
echo -e "${RED}Failed: $((TEST_COUNT - PASS_COUNT))${NC}"
echo ""

if [[ $PASS_COUNT -eq $TEST_COUNT ]]; then
    echo -e "${GREEN}🎉 All search tests passed! The search_thoughts tool is working correctly.${NC}"
    exit 0
else
    echo -e "${YELLOW}⚠️  Some search tests failed. This may indicate:${NC}"
    echo "   - No thoughts in the database yet"
    echo "   - Embedding generation issues"
    echo "   - Database connection problems"
    echo "   - Similarity threshold too high"
    echo ""
    echo -e "${YELLOW}Try adding some thoughts first with convo_think tool, then run tests again.${NC}"
    exit 1
fi
