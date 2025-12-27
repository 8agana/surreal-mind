---
date: 2025-12-27
type: audit
status: complete
auditor: Gemini CLI (3-Pro)
---

# Technical Debt & Cleanup Audit: Surreal Mind

## Executive Summary
The `surreal-mind` codebase is architecturally sound (modular `rmcp` usage), but retains significant artifacts from recent refactors (The "Lobotomy" of photography logic). The most critical debt is the presence of an entire unused module tree (`src/frameworks/`), stale tool references in schema definitions which could confuse LLM consumers, and fragile binary spawning for maintenance operations.

## High Priority Fixes

### 1. Dead Code: `src/frameworks/`
**Location:** `src/frameworks/` (entire directory), `src/lib.rs` (line 7)
**Issue:** The `frameworks` module (specifically `convo.rs`) is completely unused. The active cognitive engine (`src/cognitive/`) implements its own `Framework` trait and does not rely on this legacy code.
**Fix:** 
1. Delete `src/frameworks/` directory.
2. Remove `pub mod frameworks;` from `src/lib.rs`.
**Impact:** Reduces compile time and cognitive load; removes confusion about which "framework" system is active.

### 2. Stale Tool References in Schemas
**Location:** `src/schemas.rs` (Lines 100-101 inside `detailed_help_schema`)
**Issue:** The `detailed_help_schema` enum still lists `legacymind_update`, `memories_populate`, and `memories_moderate`. These tools have been removed from the router (`src/server/router.rs`). Presenting these as options to the LLM is a hallucination hazard.
**Fix:** Remove these strings from the `enum` list in `detailed_help_schema`.
**Impact:** Prevents the model from trying to get help for tools that don't exist.

### 3. Fragile Maintenance Ops (Binary Spawning)
**Location:** `src/tools/maintenance.rs` (Lines 362-386)
**Issue:** `handle_reembed_kg` attempts to run `cargo run --bin reembed_kg` as a subprocess. This is extremely fragile in production environments where `cargo` or the source code might not be present.
**Fix:** 
1. Refactor `src/bin/reembed_kg.rs` logic into a public function in `src/lib.rs` (e.g., `pub async fn run_reembed_kg(...)`).
2. Update `src/tools/maintenance.rs` to call this function directly.
3. (Optional) Keep a thin wrapper in `src/bin/reembed_kg.rs` if a CLI entry point is still desired.
**Impact:** significantly improves reliability of maintenance operations.

## Medium Priority Fixes

### 4. Duplicate Re-embed Logic
**Location:** `src/bin/reembed.rs` vs `src/lib.rs` (`run_reembed`)
**Issue:** `src/bin/reembed.rs` contains a standalone implementation of the re-embedding logic that duplicates code in `src/lib.rs`.
**Fix:** Refactor `src/bin/reembed.rs` to simply call `surreal_mind::run_reembed`.
**Impact:** Single source of truth for embedding logic.

### 5. Cluttered Binaries directory
**Location:** `src/bin/`
**Issue:** The directory contains likely throwaway scripts:
- `check_db_contents.rs`
- `db_check.rs`
- `simple_db_test.rs`
- `kg_dedupe_plan.rs` (Migrational?)
- `kg_apply_from_plan.rs` (Migrational?)
**Fix:** 
1. Audit these files.
2. Delete `check_db_contents.rs`, `db_check.rs`, `simple_db_test.rs`.
3. Move one-off migration scripts to a `scripts/` or `migrations/` directory if they must be kept, or delete them if the migration is done.
**Impact:** Cleaner build target list; less noise.

### 6. Unchecked Continuity Fields
**Location:** `src/tools/maintenance.rs` (Line 183)
**Issue:** `handle_ensure_continuity_fields` uses a "simple check" that the author noted "may not catch all cases".
**Fix:** Improve the checking logic to be robust, perhaps by querying the schema definition directly and parsing it, rather than relying on a loose check.
**Impact:** Ensures database integrity reliability.

## Low Priority / Observations

### 7. Dependency Usage
**Location:** `Cargo.toml`
**Observation:** `regex` and `dirs` are used, but usage is light. `regex` is used in the `convo` framework (dead code), but might be used elsewhere.
**Fix:** After deleting `src/frameworks/`, run `cargo udeps` (if available) or manually verify if `regex` is still needed. `dirs` is used in `smtop.rs` and `main.rs`, so it is valid.

### 8. Hardcoded Defaults
**Location:** `src/config.rs` (implied) / `src/server/router.rs`
**Observation:** Some defaults (like top_k=10) are hardcoded in multiple places (schemas and logic).
**Fix:** Centralize constants for default limits and values.

## Plan of Action (Recommended)
1. **Purge:** Delete `src/frameworks/` and unused binaries.
2. **Clean:** Update `src/schemas.rs`.
3. **Refactor:** Move `reembed_kg` logic to `lib.rs` and update `maintenance.rs`.
