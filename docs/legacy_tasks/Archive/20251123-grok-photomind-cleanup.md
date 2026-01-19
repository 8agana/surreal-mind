# Prompt: Remove legacy photography MCP surface from SurrealMind now that photography-mind exists

## Goal
Finish the split by removing photography MCP/tooling from this repo, leaving SurrealMind as the single-mind server. Do NOT touch the new `photography-mind` repo (already pushed). Keep SurrealMind thinking tools intact.

## Tasks
1) **Router & Schemas**
   - Confirm `list_tools`/`call_tool` in `src/server/router.rs` has no photography tools. Remove any lingering references.
   - `src/schemas.rs`: ensure detailed_help enums and any photo schemas are removed; clean comments.

2) **Modules**
   - Delete `src/tools/photography.rs` and any `mod photography` reference (e.g., `src/tools/mod.rs`).
   - Delete `src/photography/` folder if still present.
   - Remove photo-specific bins under `src/bin` that belong in photography-mind (`photography*.rs`, `reembed_photography_kg.rs`, etc.). Keep only bins needed for core mind.

3) **Config/DB**
   - Remove photo runtime fields from `src/config.rs` (photo_*). Keep brain datastore as-is.
   - Remove photo DB handle helpers from `src/server/db.rs` (connect_photo_db, etc.) if any survived.

4) **Docs**
   - Update `README.md` and `AGENTS.md` to: one mind; no photography MCP tools; point to `https://github.com/8agana/photography-mind` for ops.
   - Adjust any quickstart/tool lists accordingly.

5) **Tests**
   - Ensure `tests/tool_schemas.rs` expects only the core tools (8). Remove any photo test remnants.

6) **Sweep**
   - Ripgrep for `photography_` and clean remaining code/comments that refer to MCP photo tools (not CLI docs).
   - Keep photography CLI references only if explicitly marked as historical in changelog.

7) **Build/Verify**
   - Run `cargo check`, `cargo clippy -D warnings`, `RUN_DB_TESTS=1 cargo test --workspace`.

8) **Changelog**
   - Add an entry noting removal of photography MCP tools (single-mind) and link to photo-mind repo.

9) **Git**
   - Stage changes but do NOT push (unless instructed). If pushing, use concise commit message.

## Constraints
- Do not delete `docs/LEDGER.md` (untracked) unless instructed.
- Do not modify `photography-mind` repo here.
- Keep brain datastore support.
- Avoid touching thought data/migrations.

## Output to deliver
- Summary of files removed/edited, tests run, and any follow-ups.
