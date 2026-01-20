# Administrative Standdown: Phase 1 - Comprehensive Audit Findings

**Date:** 2026-01-19
**Auditor:** Gemini (via Antigravity)

---

## 1. Inventory

### Core Source (`src/`)

| Path | Type | Purpose | Dependencies |
|------|------|---------|--------------|
| `main.rs` | Source | Entry point. Sets up config, logging, and `SurrealMindServer`. | `surreal_mind`, `rmcp`, `tokio`, `tracing` |
| `lib.rs` | Source | Library root. Exports modules. | All submodules |
| `config.rs` | Source | Configuration logic (Typed config). | `serde`, `env` |
| `schema.rs` | Source | Database schema definitions. | `surrealdb` |

### Server (`src/server/`)

| Path | Type | Purpose |
|------|------|---------|
| `mod.rs` | Source | Server module root. |
| `db.rs` | Source | SurrealDB connection and client management. |
| `router.rs` | Source | Request routing Logic. |
| `schema.rs` | Source | Server-specific schema. |

### Tools (`src/tools/`)

| Path | Type | Purpose |
|------|------|---------|
| `mod.rs` | Source | Module root. Registers tools. |
| `thinking.rs` | Source | Module root for `thinking/` submodule (Thoughts, Plans). |
| `thinking/*` | Source | Submodules for thinking logic (`continuity`, `mode_router`). |
| `maintenance.rs` | Source | DB Maintenance tools. |
| `wander.rs` | Source | Curiosity/Exploration tool (`kg_wander`). |
| `call_*.rs` | Source | Delegation tools (`call_gem`, `call_cc`, `call_codex`). |
| `delegate_gemini.rs` | Source | **Legacy**. Likely superseded by `call_gem.rs`. |

### Cognitive (`src/cognitive/`)

| Path | Type | Purpose |
|------|------|---------|
| `mod.rs` | Source | **Cognitive Engine**. Blends frameworks (OODA, Socratic). |
| `framework.rs` | Source | Trait definitions. |
| `ooda.rs` etc. | Source | Deterministic thinking frameworks. |

### Binaries (`src/bin/`)

| Path | Type | Likely Purpose | Status |
|------|------|----------------|--------|
| `remini.rs` | Binary | "Agent-Supervised Nightly Shift". | **Active** |
| `smtop.rs` | Binary | TUI Monitoring tool. | Active |
| `kg_wander.rs` | Binary | Standalone wander utility. | Active? |
| `kg_populate.rs` | Binary | Helper to populate DB? | **Questionable** |
| `kg_embed.rs` | Binary | Helper to run embeddings? | **Questionable** |
| `admin.rs` | Binary | Admin utils? | **Questionable** |
| `import_skater_requests.py` | Script | **BUSINESS LOGIC (Photography)**. | **ORPHAN** |
| `validate_contacts.py` | Script | **BUSINESS LOGIC (Photography)**. | **ORPHAN** |

---

## 2. Orphans & Cleanup Candidates

### 🚨 Major Findings: Business Logic Pollution

The "Lobotomy" (separation of photography logic) is incomplete in the `scripts/` directory.

- **`scripts/import_skater_requests.py`**: Pure photography business logic. Parses "SkaterRequests.md".
- **`scripts/validate_contacts.py`**: Photography contact management.
- **Action**: Move to `photography-mcp` or archive/delete.

### Binaries to Review

The `src/bin/` directory is cluttered.

- `kg_populate.rs`, `kg_embed.rs`, `reembed*`: correspond to `maintenance.rs` lib functions.
- **Recommendation**: Consolidate utilities into a single CLI (e.g. `surreal-mind-cli` or subcommands on main binary) to reduce compilation targets and maintenance surface.

### Legacy/Ambiguous

- `src/tools/delegate_gemini.rs`: Is this superseded by `call_gem.rs`?
- `src/tools/detailed_help.rs`: Is this redundant with `howto.rs`?

---

## 3. Inconsistencies

### Module Structure

- **Thinking Module**: Uses `src/tools/thinking.rs` AND `src/tools/thinking/` directory. This is valid Rust 2018 but visually splitting.
- **Tools**: Flat structure (`call_*.rs`) mixed with logic modules (`thinking`).

### Code Organization

- `scripts/*.py` vs `scripts/*.sh`: Mixed tooling.
- **Tests**: `tests/*.sh` mix of curl tests and proper integration tests.

---

## 4. Questions requiring clarification

1. **`delegate_gemini.rs` vs `call_gem.rs`**: Which is the canonical implementation? The log in `main.rs` mentions `call_gem`.
2. **`kg_*` binaries**: Are these actively used in cron jobs or by `remini`? If `remini` is the new supervisor, does it call these binaries or import the library functions?
3. **`migration/`**: There is a `migration` folder in root AND `src/bin/migration.rs`. Duplicated?

---

## 5. Architecture Observations

1. **Modular Server**: The `SurrealMindServer` and `rmcp` usage indicates a mature, modular architecture.
2. **Typed Config**: Moving to `Config::load()` is a strong positive pattern.
3. **Tooling Ecosystem**: The shift to `call_*` delegation tools is evident and cleaner than previous monolithic logic.
4. **Cognitive Split**: `src/cognitive` seems to hold the "frameworks" (OODA, Socratic), while `src/tools` holds the "verbs" (Think, Rethink). This is a good separation.
