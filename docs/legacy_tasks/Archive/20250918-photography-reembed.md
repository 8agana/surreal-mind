# Implementation Plan — Photography KG Re-Embedding Binary

**Author:** Grok Code Fast 1  
**Date:** 2025-09-18  
**Executor:** Sam Atagana

## Goals
- Create a dedicated binary (`reembed_photography_kg`) to re-embed KG entities and observations in the photography namespace (`ns=photography`, `db=work`).
- Reuse core logic from `reembed_kg` to avoid duplication, adapting for photography DB connection.
- Ensure photography namespace remains isolated; no cross-contamination with main namespace.

## Requirements & Constraints
- Photography namespace must remain isolated (`ns=photography`, `db=work`).
- Reuse existing `create_embedder` and embedding logic; no new dependencies.
- Binary must support dry-run mode and limits via CLI flags (--dry-run, --limit) for consistency with reembed_kg.
- All changes must pass `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
- No regressions to main namespace or existing tools.

## Work Breakdown

1. **Create new binary file**
   - Add `src/bin/reembed_photography_kg.rs` by copying `src/bin/reembed_kg.rs`.
   - Reuse runtime hook (connect_photo_db / clone_with_db) instead of hardcoding use_ns("photography").use_db("work") to align with env vars (SURR_ENABLE_PHOTOGRAPHY, SURR_PHOTO_*).
   - Add photography-specific logging (e.g., "📸 Photography KG re-embed starting").

2. **Adapt DB connection**
   - Modify config loading to use photography-specific env vars if set, else default to photography namespace/db.
   - Ensure connection uses the runtime hook for photography DB isolation.

3. **Reuse embedder and logic**
   - Import `create_embedder` and reuse embedder creation, progress logging, and update queries.
   - Keep hygiene checks (missing, mismatched embeddings) and skip logic.
   - Adapt text generation for photography KG (entities: name + type; observations: name + description).
   - Future: Move shared logic (text builder, embedding update) into a helper to prevent divergence.

4. **Build and register binary**
   - Add `[[bin]]` entry in `Cargo.toml` if needed (likely auto-detected).
   - Test compilation: `cargo build --bin reembed_photography_kg`.
   - Update README binaries section and photography section with new binary description.
   - Update CHANGELOG: Note new binary requires SURR_ENABLE_PHOTOGRAPHY.

5. **Testing and validation**
   - Add integration test (feature-gated with `--features db_integration`) or provide concrete commands: e.g., `photography_memories --mode create` to populate, then `./reembed_photography_kg --limit 5`, verify output and main KG untouched (re-run reembed_kg).
   - Support CLI flags: `./reembed_photography_kg --dry-run` (no writes), `./reembed_photography_kg --limit 10` (limit items).
   - Confirm photography KG embeddings persist correctly and main namespace isolated.

6. **Documentation updates**
   - Add to `README.md` binaries section: "reembed_photography_kg: Re-embed photography KG entities/observations".
   - Mention in CHANGELOG: "Added photography-specific KG re-embedding binary".

## Risks & Mitigations
- **Risk:** Accidental main namespace re-embedding.
  - *Mitigation:* Reuse runtime hook for photography DB; validate with dry-run and integration tests.
- **Risk:** Code duplication introduces bugs.
  - *Mitigation:* Copy-paste reembed_kg, then modify only DB connection; reuse all other logic; plan to extract shared helpers later.
- **Risk:** Environment conflicts.
  - *Mitigation:* Align with existing photography env vars (SURR_ENABLE_PHOTOGRAPHY, SURR_PHOTO_*).

## Deliverables
- New binary `reembed_photography_kg` in `target/release/`.
- Source file `src/bin/reembed_photography_kg.rs`.
- Updated README (binaries and photography sections), CHANGELOG (with SURR_ENABLE_PHOTOGRAPHY note).
- Successful test runs with photography KG data via concrete commands and feature-gated integration tests.

---
*Ready for execution. Run binary after building to validate photography namespace embedding.*