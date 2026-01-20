# REMini CLI Consolidation Plan

**Goal**: Unify `kg_*` maintenance binaries into a single `remini` CLI.

## Current State

- `remini`: Supervisor agent (nightly shift).
- `kg_populate`: **Heavy binary** (700+ lines of extraction logic).
- `kg_embed`: Wrapper around `maintenance::run_kg_embed`.
- `kg_wander`: Wrapper around `tools::wander::Wanderer`.
- `reembed*`: Wrappers around `maintenance::run_reembed`.

## Target Architecture

### Library Structure (`src/lib.rs` & `src/maintenance/`)

Move logic out of `src/bin/` into reusable modules:

- `src/maintenance/populate.rs`: Move `kg_populate` logic here. (Biggest task)
- `src/maintenance/embed.rs`: Ensure `kg_embed` logic is here.
- `src/maintenance/wander.rs`: (Already exists as `tools::wander`? Consolidate).

### CLI Structure (`src/bin/remini.rs`)

Use `clap` with subcommands:

```rust
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>, // Default to Supervisor if None
    
    // Global flags (dry-run, ver)
}

#[derive(Subcommand)]
enum Commands {
    /// Agent supervisor (default behavior)
    Supervisor(SupervisorArgs),
    
    /// Extract entities from thoughts
    Populate(PopulateArgs),
    
    /// Generate embeddings
    Embed(EmbedArgs),
    
    /// Curiosity exploration
    Wander(WanderArgs),
    
    /// Re-calculate embeddings
    Reembed(ReembedArgs),
}
```

## Migration Steps

1. **Refactor `kg_populate`**: Extract logic to `src/maintenance/populate.rs`.
2. **Verify `kg_embed`**: Confirm wrapper status.
3. **Update `remini.rs`**: Implement `clap` subcommands.
4. **Wire Calls**: `remini populate` -> `maintenance::populate::run()`.
5. **Test**: Verify each subcommand against original binary behavior.
6. **Cleanup**: Delete `kg_*` binaries.

## Impact on `nightly_shift.sh`

The shell script currently calls `remini` (supervisor). This will remain the default or become `remini supervisor`. The script itself might need updates if it calls `kg_populate` directly (it shouldn't, `remini` logic manages that).
