# Refactor Task: De-Monolith the Photography CLI

**Objective:** Refactor the "God Object" binary `src/bin/photography.rs` into a modular, library-backed architecture within the `surreal-mind` crate.

**Why:** The current file is ~1000 lines of mixed concerns (CLI parsing, DB models, business logic, SQL strings). This makes AI assistance difficult and error-prone. We need strict separation of concerns.

## 1. The Architecture Plan

We will move logic from `src/bin/photography.rs` into a new module `src/photography/`.

**New File Structure:**
```text
src/
├── photography/
│   ├── mod.rs           # Public exports
│   ├── models.rs        # Data structures (SkaterRow, StatusRow, etc.)
│   ├── commands.rs      # The logic functions (import_roster, check_status, etc.)
│   └── utils.rs         # Helpers (format_family_id, parse_skater_names)
├── lib.rs               # Add `pub mod photography;`
└── bin/
    └── photography.rs   # Minimal CLI wrapper (Argument parsing only)
```

## 2. Execution Steps

### Step 1: Create the Module Structure
1. Create directory `src/photography/`.
2. Create `src/photography/mod.rs` (expose submodules).
3. Update `src/lib.rs` to export the module.

### Step 2: Extract Models (`models.rs`)
Move the following structs from `src/bin/photography.rs` to `src/photography/models.rs`. Ensure they derive necessary traits (`Debug`, `Serialize`, `Deserialize`) and imports (`surrealdb::sql::Thing`, etc.) are present.
- `SkaterRow`
- `StatusRow` (Ensure all new fields `ty_requested`, `ty_sent` etc. are present)
- `Family`
- `RosterRow`
- `ParsedSkater`
- `ParsedName`

### Step 3: Extract Utilities (`utils.rs`)
Move helper functions to `src/photography/utils.rs`.
- `format_family_id`: **CRITICAL:** Ensure it uses `.replace(" ", "_")` (Underscore), NOT hyphen. This was a recent bug fix.
- `parse_skater_names`
- `competition_to_id`

### Step 4: Extract Business Logic (`commands.rs`)
Move the async command implementations. They will need to accept the DB client and arguments.
- `import_roster`
- `check_status`
- `update_gallery`
- `request_ty` / `send_ty`
- `mark_sent`
- `get_email`
- `list_skaters` / `list_events`
- `competition_stats`

**Note:** You may need to update function signatures to be public and accessible from the binary.

### Step 5: Slim Down the Binary
Rewrite `src/bin/photography.rs` to:
1. Initialize the DB connection.
2. Parse `Cli` args (keep `clap` structs here or move to `src/photography/cli.rs` if you prefer, but keeping CLI args in binary is acceptable for now).
3. Dispatch arguments to the functions in `src::photography::commands`.

## 3. Validation
- **Compile Check:** Run `cargo check` frequently.
- **Behavior Check:** The behavior must remain identical.
- **Dependencies:** Ensure `Cargo.toml` dependencies (`prettytable`, `csv`, `serde`) are available to the library target (they are shared, so this should be fine).

## 4. Config cleanup (Bonus)
If you see hardcoded strings like `"2025_fall_fling"` repeated often, consider defining them as `pub const DEFAULT_COMP: &str = "2025_fall_fling";` in `mod.rs` or a `config.rs` to reduce magic strings.

## 5. Clarifications & Answers to Grok

**1. Missing Business Logic Functions:**
YES. All business logic must move. This includes:
- `record_purchase`
- `query_skater`
- `list_events_for_skater`
- `show_event`
basically *every* async function except `main`.

**2. Structs for Parsed Data:**
`ParsedSkater` and `ParsedName` are defined at the bottom of the current `src/bin/photography.rs`. Move them to `models.rs` alongside the DB rows.

**3. Database Client Type:**
Keep the specific type `&Surreal<surrealdb::engine::remote::ws::Client>`. Do not introduce generics or traits yet. Keep it simple and matching the current implementation.

**4. Error Handling:**
Stick to `anyhow::Result`. This is a CLI tool; bubbling errors to `main` is the desired behavior.

**5. Imports:**
Standard imports (`serde`, `surrealdb::sql::Thing`, `anyhow`) are sufficient. Ensure `prettytable` is imported in `commands.rs` for the display functions.

**6. Config Constants:**
YES. Please create constants in `mod.rs` (or `config.rs` if you prefer) for:
- `DEFAULT_COMP` ("2025_fall_fling")
- `NAMESPACE` ("photography")
- `DB_NAME` ("ops")

**7. CLI Structs:**
Keep `clap` structs (`Cli`, `Commands`, `ListCommands`, etc.) in `src/bin/photography.rs`. The binary defines the interface; the library defines the implementation.

**8. Function Signatures:**
Yes, make the moved functions `pub`. Keep borrowing (`&str`) where possible to minimize churn/cloning.

**9. Testing:**
Do NOT add new unit tests in this pass. Focus strictly on the refactor (moving code) to minimize variables. We verify behavior by compiling and running the CLI manually (or via the `check-status` tests we just ran).

**10. Dependencies:**
`surreal-mind` is a single crate. All dependencies in `Cargo.toml` are available to `src/lib.rs` and its modules. No `Cargo.toml` changes needed.
