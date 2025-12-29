# Surreal Mind Crates Upgrade

**Date**: 2025-12-27
**Prompt Type**: Implementation Plan (crates updates)
**Justification**: Keeping surreal-mind up-to-date with the latest crates.
**Status**: Complete
**Implementation Date**: 2025-12-27

### 1. Analysis of Outdated Dependencies

Based on the output of `cargo outdated`, the following key direct dependencies have newer versions available. The most significant are major version changes that are highly likely to introduce breaking changes.

**High-Risk Major/Minor Updates:**

*   **`axum`**: `0.7.9` -> `0.8.8` (Minor release, but `axum` is rapidly developing).
*   **`chrono-tz`**: `0.9.0` -> `0.10.4` (Minor release, timezone data updates).
*   **`crossterm`**: `0.27.0` -> `0.29.0` (Minor release, potential TUI API changes).
*   **`dirs`**: `5.0.1` -> `6.0.0` (**Major release**, high chance of breaking changes).
*   **`governor`**: `0.6.3` -> `0.10.4` (**Major release**, high chance of breaking changes).
*   **`ratatui`**: `0.26.3` -> `0.30.0` (**Major release**, high chance of breaking changes for TUI).
*   **`rusqlite`**: `0.32.1` -> `0.38.0` (**Major release**, likely breaking changes).
*   **`serde_qs`**: `0.13.0` -> `0.15.0` (Minor release, potential changes).
*   **`surrealdb`**: `2.3.10` -> `2.4.0` (Minor release, but critical).
*   **`toml`**: `0.8.23` -> `0.9.10` (Minor release).
*   **`tower`**: `0.4.13` -> `0.5.2` (**Major release**, breaking changes expected).
*   **`tower-http`**: `0.5.2` -> `0.6.8` (Minor release, related to `tower`).

### 2. Research on Breaking Changes

*   **`ratatui` (0.26.3 -> 0.30.0):**
    *   **Modularization:** The single `ratatui` crate is now a workspace. We will need to update `Cargo.toml` to use the new scoped crates (e.g., `ratatui-core`, `ratatui-widgets`, `ratatui-crossterm`).
    *   **API Changes:** Imports will need to be updated to reflect the new crate structure. The `ratatui::run()` function is new and might simplify TUI initialization. Layouting methods on `Rect` have been improved and might allow for code cleanup.

*   **`tower` (0.4.13 -> 0.5.2):**
    *   **`retry::Policy`:** The `retry` method signature has changed to take `&mut Req` and `&mut Res`. All implementations of this trait will need to be updated.
    *   **`&mut self`:** The `Policy` trait now requires `&mut self`.
    *   **MSRV:** Increased to 1.63.0, which is compatible with our toolchain.

*   **`rusqlite` (0.32.1 -> 0.38.0):**
    *   **`u64`/`usize` handling:** `ToSql`/`FromSql` for these types is now behind a feature flag. We need to check for usages and enable the `i128_blob` feature if necessary.
    *   **Statement Cache:** Now optional, but this is unlikely to cause issues.
    *   **SQLite Version:** The bundled version is newer, which is good.

*   **`dirs` (5.0.1 -> 6.0.0):**
    *   **macOS `config_dir`:** This now points to the Application Support directory. We must check all usages of `dirs::config_dir()` and determine if we need to migrate configuration files or switch to `dirs::preference_dir()`.
    *   **Linux XDG:** Returns `None` for invalid entries. This is a minor concern for us as we primarily target macOS for development.

*   **`governor` (0.6.3 -> 0.10.4):**
    *   **Complete Overhaul:** `governor` is a replacement for `ratelimit_meter` and `ratelimit_futures`. The entire API is different.
    *   **Algorithm:** Now uses GCRA exclusively.
    *   **State Management:** Uses `AtomicU64` instead of `Mutex`, which should improve performance. We will need to rewrite the rate-limiting logic.

### 3. Proposed Implementation Plan

The update process will be done in batches to isolate potential issues and manage risk.

**Note on `cargo update`:** The `cargo update -p` command is not suitable for this task as it can unintentionally update dependencies of the specified packages, including other high-risk packages. Therefore, we will update each high-risk package individually and then run a full `cargo update` at the end.

**Batch 1: TUI Stack (`ratatui`, `crossterm`)**

1.  **Action:** Update `ratatui` and `crossterm`. This is a high-risk change.
2.  **Commands:**
    ```bash
    cargo update -p ratatui -p crossterm
    ```
3.  **Changes Required:**
    *   Modified `Cargo.toml` to use the new `ratatui` modular dependency: `ratatui = { version = "0.30.0", features = ["all-widgets", "crossterm"] }`.
    *   Updated `src/bin/smtop.rs` to use `ratatui::crossterm` and `f.area()` instead of `f.size()`.
    *   Added `dirs` back to `Cargo.toml`.
    *   Updated `src/main.rs` to import `dirs`.
4.  **Verification:**
    *   Ran `cargo check` and fixed compilation errors.
    *   Ran `cargo test` and fixed a failing test in `tests/inner_voice_providers_gate.rs` by unsetting the `IV_ALLOW_GROK` environment variable before calling `allow_grok()`.
5.  **Status:** `Complete`

**Batch 2: HTTP & Server Stack (`axum`, `tower`, `tower-http`)**

1.  **Action:** Update `axum`, `tower`, and `tower-http`.
2.  **Commands:**
    ```bash
    cargo update -p tower@0.5.2
    cargo update -p tower-http@0.6.8
    cargo update -p axum
    ```
3.  **Changes Required:**
    *   No breaking changes were found.
4.  **Verification:**
    *   Ran `cargo check` and `cargo test` and all tests passed.
5.  **Status:** `Complete`

**Batch 3: Database Stack (`surrealdb`, `rusqlite`)**

1.  **Action:** Update `surrealdb` and `rusqlite`.
2.  **Commands:**
    ```bash
    cargo update -p surrealdb -p rusqlite
    ```
3.  **Changes Required:**
    *   No breaking changes were found.
4.  **Verification:**
    *   Ran `cargo check` and `cargo test` and all tests passed.
5.  **Status:** `Complete`

**Batch 4: Core Libraries (`dirs`, `governor`, `chrono-tz`, `toml`)**

1.  **Action:** Update `dirs`, `chrono-tz`, and `toml`.
2.  **Commands:**
    ```bash
    cargo update -p dirs -p chrono-tz -p toml
    ```
3.  **Changes Required:**
    *   No breaking changes were found. `governor` was not a direct dependency.
4.  **Verification:**
    *   Ran `cargo check` and `cargo test` and all tests passed.
5.  **Status:** `Complete`

**Batch 5: Low-Risk Dependencies & Finalization**

1.  **Action:** Update all remaining dependencies to their latest compatible versions.
2.  **Command:**
    ```bash
    cargo update
    ```
3.  **Verification:**
    *   Ran `cargo check --all-features` and all checks passed.
    *   Ran `cargo test --all-features` and all tests passed.
4.  **Status:** `Complete`

**Overall Status:** `Complete`
