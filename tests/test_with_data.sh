#!/bin/bash

# Test script that first creates thoughts, then tests search functionality
echo "=== Testing think_search with Data ==="

# Create several test thoughts first
echo "1. Creating test thoughts..."
export SURR_EMBED_PROVIDER=candle
{
    # Initialize
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'

    # Create thought 1
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"think_convo","arguments":{"content":"Artificial intelligence is transforming the world through machine learning and deep neural networks","injection_scale":3,"significance":0.8}}}'
    sleep 1

    # Create thought 2
    echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"think_convo","arguments":{"content":"Machine learning algorithms can learn patterns from data without explicit programming","injection_scale":2,"significance":0.7}}}'
    sleep 1

    # Create thought 3
    echo '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"think_convo","arguments":{"content":"Deep learning uses neural networks with many layers to solve complex problems","injection_scale":3,"significance":0.9}}}'
    sleep 2

} | cargo run --bin surreal-mind 2>/dev/null | grep -E '"thought_id"|"error"'

echo ""
echo "2. Testing search with exact match..."
{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"think_search","arguments":{"content":"Artificial intelligence machine learning","top_k":5,"sim_thresh":0.3}}}'
    sleep 2
} | cargo run --bin surreal-mind 2>/dev/null | grep -E '"total"|"results"|"error"'

echo ""
echo "3. Testing search with broader terms..."
{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"think_search","arguments":{"content":"neural networks deep learning","top_k":5,"sim_thresh":0.2}}}'
    sleep 2
} | cargo run --bin surreal-mind 2>/dev/null | grep -E '"total"|"results"|"error"'

echo ""
echo "4. Testing search with very low threshold..."
{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"think_search","arguments":{"content":"technology algorithms","top_k":5,"sim_thresh":0.1}}}'
    sleep 2
} | cargo run --bin surreal-mind 2>/dev/null | grep -E '"total"|"results"|"error"'

echo ""
echo "=== Test complete ==="
echo "If you see 'total':0 in results, check:"
echo "  - Database connection"
echo "  - Embedding generation"
echo "  - Thought storage"
