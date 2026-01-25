---
description: How to ensure documentation stays in sync with code changes
---

# Workflow: Documentation Synchronization

**Trigger:** You have modified code, configuration, or architecture.
**Mandate:** "Documentation is Code." You cannot commit a feature without its shadow (docs).

## 1. Dependency Check

If you modified `Cargo.toml`:

1. Run `cargo tree` to inspect new dependencies.
2. Update `docs/DEPENDENCIES.md` with:
    * New crates (and why).
    * New system requirements (e.g., `openssl`, `protoc`).

## 2. Configuration Check

If you modified configuration structs (e.g., `src/config.rs`) or environment variables:

1. Update `.env.example` with new keys.
2. Update `README.md` "Configuration Quick Reference" section.
3. Check `deployment/` manifests if applicable (Docker, launchd).

## 3. Tool Surface Check

If you added, removed, or renamed a Tool in `src/tools/`:

1. Update the "Tool Surface" table in `README.md`.
2. Ensure columns (Description, Arguments) are accurate.

## 4. Architecture & Doctrine Check

If you changed core logic (e.g., "how we think" or "how we persist"):

1. Check `GEMINI.md` (Cognitive Kernel section).
2. Ensure broadly applicable changes are reflected in the operational doctrine.

## 5. Knowledge Graph Check

If you implemented a new pattern or fixed a major bug:

1. Check `docs/patterns/` (if it exists) or create a brief note.
2. Use the `write_memory` tool (if available) to log a persistent lesson for future agents.

## 6. Final Polish

* Run `cargo fmt` (documentation often lives in doc-comments too).
* Verify links in `README.md` still work.
