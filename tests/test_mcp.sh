#!/bin/bash

# Start the server and keep stdin open
(
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}'
    sleep 0.1
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    sleep 0.1
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
    sleep 1
) | cargo run 2>&1 | grep -E '"(result|error)"' | jq '.'
