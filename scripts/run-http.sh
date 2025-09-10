#!/bin/zsh
set -euo pipefail
export SURR_TRANSPORT=http
export SURR_HTTP_BIND=127.0.0.1:8787
# Allow query token for Claude Desktop simplicity
export SURR_ALLOW_TOKEN_IN_URL=1
# Avoid TOML parsing issues; rely on defaults + env
# Use project config if present
if [ -f "$HOME/Projects/LegacyMind/surreal-mind/surreal_mind.toml" ]; then
  export SURREAL_MIND_CONFIG="$HOME/Projects/LegacyMind/surreal-mind/surreal_mind.toml"
fi
# Optional: quiet logs
export RUST_LOG=surreal_mind=info,rmcp=info
# Ensure token exists
if [ ! -s "$HOME/.surr_token" ]; then
  uuidgen | tr -d '\n' > "$HOME/.surr_token"
  chmod 600 "$HOME/.surr_token"
fi
# Move to repo so relative paths work
cd "$HOME/Projects/LegacyMind/surreal-mind"
# Load local env (DB, OPENAI_API_KEY, etc.) if present
set +u
if [ -f .env ]; then
  set -a; source .env; set +a
fi
set -u
# Exec compiled binary (must be built once)
exec "$HOME/Projects/LegacyMind/surreal-mind/target/release/surreal-mind"
