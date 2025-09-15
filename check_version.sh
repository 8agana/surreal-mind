#!/bin/bash
# Quick version check for surreal-mind binary

echo "=== SurrealMind Version Check ==="
echo ""

# Check compiled rmcp version
echo "RMCP version in binary:"
strings /Users/samuelatagana/Projects/LegacyMind/surreal-mind/target/release/surreal-mind | grep -oE "rmcp-0\.[0-9]+\.[0-9]+" | sort -u

echo ""
echo "Build timestamp:"
stat -f "Built: %Sm" -t "%Y-%m-%d %H:%M:%S" /Users/samuelatagana/Projects/LegacyMind/surreal-mind/target/release/surreal-mind

echo ""
echo "Cargo.toml version:"
grep "^version" /Users/samuelatagana/Projects/LegacyMind/surreal-mind/Cargo.toml

echo ""
echo "Running process info:"
ps aux | grep -E "[s]urreal-mind" | head -1

echo ""
echo "Last restart:"
tail -1 ~/Library/Logs/surreal-mind.out.log | grep "Starting Surreal Mind"