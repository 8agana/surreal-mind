**Project:** `surreal-mind`
**Context:** The `photography` CLI currently requires exact or `CONTAINS` matching for competition names, which is brittle. We want to support fuzzy matching to handle typos and partial inputs.

**Objective:**
Implement a `resolve_competition` helper function that uses fuzzy matching to find the correct competition ID from user input.

**Instructions:**
1.  **Update `src/photography/utils.rs`:**
    *   Implement `pub async fn resolve_competition(db: &Surreal<Client>, input: &str) -> Result<String>`.
    *   **Logic:**
        1.  Query all competition names from the DB: `SELECT name FROM competition`.
        2.  **Strategy 1 (Exact/Contains):** If `input` matches exactly or is a substring of a name (case-insensitive), return that name.
        3.  **Strategy 2 (Fuzzy):** Use the `strsim` crate (already in `Cargo.toml`) to calculate Jaro-Winkler similarity.
        4.  If the best match has a score > 0.8, return it.
        5.  If ambiguous or no match, return an `anyhow::Error` listing available options.
    *   **Return Value:** The function should return the *canonical name* of the competition (e.g., "2025 Pony Express") or its ID string if easier, to be used by downstream commands.

2.  **Integrate into Commands (`src/photography/commands.rs`):**
    *   Update `check_status`, `import_roster`, `mark_sent`, `request_ty`, `send_ty`, `record_purchase`, `list_events`, `competition_stats`, `list_skaters` (if it takes competition), and `set_status`.
    *   Replace manual `comp.to_lowercase()` or raw string usage with:
        ```rust
        let resolved_comp = resolve_competition(db, comp_name).await?;
        ```
    *   Ensure the downstream queries use this resolved name.

**Validation Criteria:**
- `cargo build --bin photography` must pass.
- `photography check-status "pony"` should successfully resolve to "2025 Pony Express".
- `photography check-status "fal fling"` (typo) should resolve to "2025 Fall Fling".
