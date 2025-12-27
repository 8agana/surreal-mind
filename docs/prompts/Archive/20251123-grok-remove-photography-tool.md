Goal: purge leftover photography tools from SurrealMind binary and server surface.

Context:
- Code cleanup already removed photography modules/bins/config, but the shipped binary still reports `photography_think`/`photography_search` (strings found in `src/tools/detailed_help.rs`).
- SurrealMind launchd service: `dev.legacymind.surreal-mind` (plist in `~/Library/LaunchAgents`).

Tasks for Grok:
1) Remove any remaining photography tool entries from code (likely `src/tools/detailed_help.rs`; search for `photography_`). Ensure only 8 core tools remain.
2) Run: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --test tool_schemas` (full tests optional if slow).
3) Build release: `cargo build --release`.
4) Restart service to load new binary:
   - `launchctl bootout gui/501/dev.legacymind.surreal-mind` (ignore if not loaded)
   - `launchctl bootstrap gui/501 ~/Library/LaunchAgents/dev.legacymind.surreal-mind.plist`
   - `launchctl kickstart -k gui/501/dev.legacymind.surreal-mind`
5) Verify runtime surface: `strings target/release/surreal-mind | grep photography_` should return nothing; optionally call `list_tools` via rmcp to confirm 8 tools.
6) Add a short note to `CHANGELOG.md` under 2025-11-24 entry: “Removed lingering photography tool metadata from binary; rebuilt and restarted service.”

Acceptance:
- No `photography_*` strings in release binary or code.
- `cargo clippy` clean; `cargo test --test tool_schemas` passes.
- Service running with new binary.
