#!/bin/bash
# Stub Gemini executable for testing cancellation behavior
# Simulates a long-running Gemini CLI process that outputs "tick" every 200ms

set -e

DURATION="${1:-10}"  # Default 10 seconds if not specified
START=$(date +%s%N | cut -b1-13)  # milliseconds

while true; do
    NOW=$(date +%s%N | cut -b1-13)
    ELAPSED=$((NOW - START))

    if [ "$ELAPSED" -ge "$((DURATION * 1000))" ]; then
        echo '{"status":"complete","ticks":'$((ELAPSED / 200))'}'
        break
    fi

    echo "tick"
    sleep 0.2
done
