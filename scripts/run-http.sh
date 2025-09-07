#!/bin/zsh
set -euo pipefail
export SURR_TRANSPORT=http
export SURR_HTTP_BIND=127.0.0.1:8787
# Allow query token for Claude Desktop simplicity
export SURR_ALLOW_TOKEN_IN_URL=1
# Avoid TOML parsing issues; rely on defaults + env
export SURREAL_MIND_CONFIG=/nonexistent-sm-config.toml
# Optional: quiet logs
export RUST_LOG=surreal_mind=info,rmcp=info
# Ensure token exists
if [ ! -s "$HOME/.surr_token" ]; then
  uuidgen | tr -d '\n' > "$HOME/.surr_token"
  chmod 600 "$HOME/.surr_token"
fi
# Move to repo so relative paths work
cd "$HOME/Projects/LegacyMind/surreal-mind"
# Exec compiled binary (must be built once)
exec "$HOME/Projects/LegacyMind/surreal-mind/target/release/surreal-mind"
