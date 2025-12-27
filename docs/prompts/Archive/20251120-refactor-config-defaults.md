**Project:** `surreal-mind`
**Context:** Following the major refactor of `photography.rs` into the `src/photography/` module.

**Objective:**
Eliminate the hardcoded `"2025_fall_fling"` default value from `src/bin/photography.rs` and centralize it into the new `photography` library module.

**Instructions:**
1.  **Create Constant:** In `surreal-mind/src/photography/mod.rs`, add a public constant:
    ```rust
    pub const DEFAULT_COMPETITION: &str = "2025_fall_fling";
    ```
2.  **Update Binary:** In `surreal-mind/src/bin/photography.rs`, import and use this constant for all relevant `clap` default values.
    *   Import the constant: `use surreal_mind::photography::DEFAULT_COMPETITION;`
    *   Modify the `#[arg(default_value = ...)]` attributes in the `Commands` enum for `MarkSent`, `RequestTy`, `SendTy`, `RecordPurchase`, and `CheckStatus` to use `DEFAULT_COMPETITION`.

**Validation Criteria:**
- The string `"2025_fall_fling"` must be completely removed from `src/bin/photography.rs`.
- `cargo build --bin photography` must pass.
- Running `photography check-status --help` should still show "2025_fall_fling" as the default value for the `COMPETITION` argument.
