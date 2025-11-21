#!/bin/bash
# Cleanup duplicate family_competition edges for Pony Express
# Created: 2025-11-20 by CC
# Issue: Old duplicate edges from before DELETE+RELATE fix

set -e

PHOTO_BIN="$HOME/Projects/LegacyMind/surreal-mind/target/release/photography"

echo "🧹 Cleaning up Pony Express duplicate edges..."
echo ""

# Step 1: Delete ALL family_competition edges for Pony Express
echo "Step 1: Deleting all family_competition edges for Pony Express..."
curl -s -u 'root:root' -X POST http://localhost:8000/sql \
  -H 'Content-Type: text/plain' \
  -H 'NS: photography' \
  -H 'DB: ops' \
  --data-binary "DELETE family_competition WHERE out = type::thing('competition', 'pony_express');" > /dev/null

echo "✅ All edges deleted"
echo ""

# Step 2: Re-mark families that should be sent
echo "Step 2: Re-marking sent families..."
SENT_FAMILIES=("Mellender" "Moritz" "Williams" "Savoy" "Rodriguez")

for family in "${SENT_FAMILIES[@]}"; do
  echo "  Marking $family as sent..."
  $PHOTO_BIN mark-sent "$family" "Pony Express"
done

echo ""
echo "✅ Cleanup complete!"
echo ""
echo "Verification:"
$PHOTO_BIN check-status pony | grep -E '(mellender|moritz|williams|savoy|rodriguez)'
