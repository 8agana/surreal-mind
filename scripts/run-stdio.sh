#!/bin/zsh
set -euo pipefail
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
export SURR_TRANSPORT=stdio

# Use project config if present
if [ -f "$HOME/Projects/LegacyMind/surreal-mind/surreal_mind.toml" ]; then
  export SURREAL_MIND_CONFIG="$HOME/Projects/LegacyMind/surreal-mind/surreal_mind.toml"
fi

# Suppress all logging for stdio mode - stdout must be clean JSON-RPC only
export RUST_LOG=off

# Move to repo so relative paths work
cd "$HOME/Projects/LegacyMind/surreal-mind"

# Load local env (DB, OPENAI_API_KEY, etc.)
set +u
if [ -f .env ]; then
  set -a; source .env; set +a
fi
set -u

# Exec compiled binary
exec "$HOME/Projects/LegacyMind/surreal-mind/target/release/surreal-mind"
