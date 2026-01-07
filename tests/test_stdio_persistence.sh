#!/bin/bash
# Test script for stdio persistence bug
# Created: 2025-09-14 19:39 CDT
# Purpose: Verify if rmcp 0.6.4 fixes the stdio persistence issue

echo "=== SurrealMind Stdio Persistence Test ==="
echo "rmcp version: 0.6.4"
echo "Test time: $(date)"
echo ""

# Set environment for stdio transport
export SURR_TRANSPORT=stdio
export SURR_EMBED_PROVIDER=openai
export OPENAI_API_KEY=$(cat ~/LegacyMind_Vault/Secure/API_Keys.md | grep OPENAI_API_KEY | cut -d'=' -f2)

# Create a unique test thought
TEST_ID="stdio-test-$(date +%Y%m%d-%H%M%S)"
TEST_CONTENT="Testing stdio persistence with rmcp 0.6.4 at $(date)"

echo "1. Creating test thought via stdio..."
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"think","arguments":{"content":"'$TEST_CONTENT'","hint":"debug"}},"id":1}' | /Users/samuelatagana/Projects/LegacyMind/surreal-mind/target/release/surreal-mind

echo ""
echo "2. Waiting 2 seconds for DB write..."
sleep 2

echo ""
echo "3. Searching for the thought..."
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"thoughts_content":"rmcp 0.6.4","top_k_thoughts":5}},"id":2}' | /Users/samuelatagana/Projects/LegacyMind/surreal-mind/target/release/surreal-mind

echo ""
echo "=== Test Complete ==="
echo "If the thought appears in search results, stdio persistence is FIXED!"
echo "If not, the bug persists in rmcp 0.6.4"