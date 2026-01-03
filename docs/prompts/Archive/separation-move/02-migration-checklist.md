# Migration Checklist — Namespace Separation

## Pre-Work
- [ ] Confirm SurrealDB version matches production (document exact commit / binary).
- [ ] Take a full snapshot (`surreal export`) of the current database.
- [ ] Inventory existing tables (`INFO FOR DB; INFO FOR KV;`) to map current data to new schema.
- [ ] Freeze writes from MCP tools during migration window (disable automation or run in read-only mode).

## Step 1 — Namespace & Database Creation
- [ ] For each agent (`codex`, `gemini`, `claude`, ...):
  - [ ] `USE NS <agent>;`
  - [ ] `DEFINE DATABASE brain;`
  - [ ] `DEFINE DATABASE memories;`
  - [ ] `DEFINE DATABASE thoughts;`
  - [ ] Apply schema definitions from `01-schema.md`.
- [ ] Create `federation` namespace and `brain` database with shared schema.

## Step 2 — Brain Migration
- [ ] Parse existing Markdown brain files into structured sections.
- [ ] Insert entries into `<agent>.brain` (preserving source order and metadata).
- [ ] Populate `federation.brain` with cross-agent directives (LegacyMind charter, SurrealMind ops, Sam profile, etc.).
- [ ] Validate queries (e.g., fetch section by name, tag) return expected content.

## Step 3 — Memories Migration
- [ ] Extract current knowledge graph records (if any) and map to `node` / `edge` tables.
- [ ] Write helper script to translate old IDs to new structure.
- [ ] Insert per-agent memories under their namespace.
- [ ] Smoke-test graph queries (e.g., find all memories connected to Pony Express project).

## Step 4 — Thoughts Migration
- [ ] Export existing thought logs.
- [ ] Normalize fields (`content`, `tags`, `created_at`, `tool_version`).
- [ ] Bulk insert into `<agent>.thoughts`.
- [ ] Verify counts match pre-migration totals.

## Step 5 — Tooling Updates
- [ ] Update MCP config files with `"agent": "<name>"`.
- [ ] Modify Legacymind tools to read the identity flag and set SurrealDB namespace before executing queries.
- [ ] Add `scope` parameter to search tools (`own` default, `all` optional).
- [ ] Implement `legacymind_brain` CRUD endpoint.
- [ ] Remove redundant photography-specific tools and update photography MCP to use generic ones.

## Step 6 — Testing
- [ ] Run unit/integration tests for all Legacymind tools (reads/writes/scopes).
- [ ] Manual spot checks:
  - [ ] Retrieve Codex brain entry.
  - [ ] Search Codex memories (scope=own).
  - [ ] Cross-agent search (scope=all).
  - [ ] Update federation brain entry and verify from another agent.
- [ ] Re-enable automation and monitor logs for namespace errors.

## Step 7 — Documentation & Handover
- [ ] Update SurrealMind README / docs with new architecture.
- [ ] Add SOP for updating federation brain entries (review process).
- [ ] Notify all agents of new storage locations and tooling flags.
- [ ] Archive the migration scripts + snapshots for audit trail.
