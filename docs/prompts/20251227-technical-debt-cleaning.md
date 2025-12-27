---
date: 2025-12-27
type: Consolidated Technical Audit
status: Complete
scope: surreal-mind codebase
focus: Technical debt, dead code, cleanup opportunities
sourced from:
- docs/prompts/20251227-technical-debt-cleaning0.md
- docs/prompts/20251227-technical-debt-cleaning1.md
- docs/prompts/20251227-technical-debt-cleaning2.md
- docs/prompts/20251227-technical-debt-cleaning3.md
- docs/prompts/20251227-technical-debt-cleaning4.md
- docs/prompts/20251227-technical-debt-cleaning5.md
compiled by: Gemini CLI
---

# Consolidated SurrealMind Technical Debt Audit

## Executive Summary
A comprehensive review of the `surreal-mind` codebase reveals a generally healthy, modular architecture (specifically the `rmcp` implementation). However, significant artifacts remain from recent refactors (the removal of photography logic and legacy tools).

**Key Stats:**
- **Dead Code:** ~800-1000 LOC identified for removal.
- **Critical Issues:** Stale schema definitions advertising non-existent tools to LLM clients.
- **Cleanup Targets:** 7+ unused modules, 5+ obsolete binaries, and several unused dependencies.

This report consolidates findings from 6 independent audits into a single actionable plan.

---

## 1. High Priority (Critical Cleanup)

### 1.1 Stale Tool References in Schemas (Hallucination Hazard)
**Issue:** The API schemas still advertise tools that have been removed (`legacymind_update`, `memories_populate`, `memories_moderate`), causing potential hallucinations where clients try to call non-existent tools.
**Locations:**
- `src/schemas.rs`: `detailed_help_schema` enum (Lines ~94-105).
- `src/schemas.rs`: Unused schema functions `convo_think_schema`, `search_thoughts_schema`, `kg_search_schema`.
- `src/tools/detailed_help.rs`: References to legacy aliases.
- `docs/AGENTS/tools.md` & `README.md`: Outdated tool lists.
**Fix:** Remove these strings from the enums and delete the unused schema functions. Update documentation to match the active tool roster (9 tools).

### 1.2 Duplicate & Dead Logic in Inner Voice
**Issue:** A complete, 55-line duplicate implementation of `grok_call` exists in `src/tools/inner_voice.rs` (marked `#[allow(dead_code)]`) alongside the active one in `src/tools/inner_voice/providers.rs`.
**Fix:** Delete the dead implementation in `inner_voice.rs`.

### 1.3 Unsafe Test Code
**Issue:** `tests/inner_voice_providers_gate.rs` contains an unsafe function call (`env::remove_var`) that triggers compilation errors or warnings in recent Rust editions/clippy configurations.
**Fix:** Ensure the unsafe block is correctly placed or update the test to handle environment variables safely.

### 1.4 Fragile Binary Spawning
**Issue:** `src/tools/maintenance.rs` attempts to spawn `cargo run --bin reembed_kg` for maintenance tasks. This is fragile in production.
**Fix:** Refactor `reembed_kg` logic into a public library function (`src/lib.rs`) and call it directly from the tool handler.

---

## 2. Medium Priority (Dead Modules & Cleanup)

### 2.1 Unused Modules (The "Graveyard")
Several modules are no longer used or were never fully integrated.
**Recommendation:** Delete or Archive.

| Module | Location | Status |
|--------|----------|--------|
| **Frameworks** | `src/frameworks/` (entire dir) | **DEAD**. Replaced by `src/cognitive/`. Delete. |
| **Flavor** | `src/flavor.rs` | Unused. Delete. |
| **Gemini Client** | `src/gemini.rs` | Unused (replaced by `inner_voice`). Delete. |
| **Sessions** | `src/sessions.rs` | Unused. Delete `tool_sessions` table definition. |
| **Prompt Metrics** | `src/prompt_metrics.rs` | Unused infrastructure. Archive/Delete. |
| **Prompt Critiques**| `src/prompt_critiques.rs` | Unused infrastructure. Archive/Delete. |
| **Prompts Registry**| `src/prompts.rs` | Unused. Archive/Delete. |
| **KG Extractor** | `src/kg_extractor.rs` | Appears unused. Verify and Delete. |

### 2.2 Configuration Debt
- **SubmodeConfig:** `src/config.rs` contains a deprecated `SubmodeConfig` struct and `get_submode` method.
- **OrbitalWeights:** Unused struct in `src/config.rs`.
- **Hardcoded Defaults:** `src/server/db.rs` has hardcoded candidate pool sizes for tools.
- **Fix:** Remove deprecated structs; move hardcoded values to `surreal_mind.toml`.

### 2.3 Obsolete Binaries
The `src/bin/` directory contains several one-off debug scripts.
- **Delete:** `check_db_contents.rs`, `simple_db_test.rs`, `sanity_cosine.rs`, `db_check.rs`.
- **Keep:** `smtop.rs`, `reembed_kg.rs` (refactor), `migration.rs`.

---

## 3. Low Priority (Polish & Nits)

- **Dead Code Allows:** `src/server/db.rs` has `#[allow(dead_code)]` on `cosine_similarity` which *is* used. Remove the attribute.
- **Unused Imports:** `use dirs;` in `main.rs` and `smtop.rs` is redundant.
- **Deprecation Warning:** `smtop.rs` uses `Frame::size()` (deprecated) instead of `Frame::area()`.
- **Stale Logs:** `src/main.rs` logs a hardcoded tool count that may be incorrect.
- **Docs:** `// Debug` comments that act as section headers should be clarified.

---

## 4. Dependency Analysis

The following dependencies in `Cargo.toml` appear to be unused or candidates for removal:

1.  **`strsim`**: Unused.
2.  **`chrono-tz`**: Unused.
3.  **`rmp-serde`**: Likely unused (MessagePack).
4.  **`rusqlite`**: Likely unused (Legacy?).
5.  **`serde_qs`**: Likely unused.
6.  **`time`**: Redundant (project uses `chrono`).

**Action:** Run `cargo udeps` to confirm and remove.

---

## Recommended Cleanup Plan

1.  **Immediate (Safety & Correctness):**
    - Clean `src/schemas.rs` and `detailed_help.rs` (Fix tool hallucinations).
    - Remove `src/frameworks/` and `src/flavor.rs` (Major dead code).
    - Fix `tests/inner_voice_providers_gate.rs`.

2.  **Secondary (Debt Reduction):**
    - Remove `src/gemini.rs`, `src/sessions.rs`, and prompt modules.
    - Delete obsolete binaries.
    - Prune `Cargo.toml`.

3.  **Tertiary (Refactoring):**
    - Refactor `reembed_kg` logic to be library-callable.
    - Centralize configuration constants.