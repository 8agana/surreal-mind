# MCP Tooling Updates

## Config Changes
- Add `"agent": "<name>"` to each Legacymind MCP entry (e.g., `codex`, `gemini`).
- Optional: `default_scope` property (`own` vs `all`) if some tools need cross-namespace reads by default.

## Tool Behavior

### legacymind_think / photography_think
- On invocation:
  1. Read `agent` from config.
  2. `USE NS <agent>; USE DB thoughts;`
  3. Insert new `thought` record.
- Append `namespace` metadata to the response for logging.

### legacymind_search
- Parameters:
  - `query` (string)
  - `scope` (enum: `own` default, `all` optional)
- Execution:
  - `own`: search `<agent>.brain`, `<agent>.memories`, `<agent>.thoughts`, plus `federation.brain`.
  - `all`: iterate known agent namespaces + federation.
- Return payload should include `namespace` + `db` for each hit.

### legacymind_memory (write)
- Accepts `node`, `edge`, or structured payload.
- Routes to `<agent>.memories`.
- Optionally supports `namespace` override for federation writes (with explicit permission).

### legacymind_brain (new)
- Methods:
  - `get(section|tag|id, namespace?)`
  - `list(tag?, namespace?)`
  - `upsert(section, body, tag?, namespace?)`
  - `delete(id, namespace?)` (restricted; require confirmation)
- Defaults to caller namespace; `namespace=federation` allowed for shared updates.

### legacy cleanup
- Remove old photography-specific MCP entries; update photography MCP CLI to call generic Legacymind tools.
- Ensure all obsolete tables are dropped after migration (document in changelog).

## Security / Permissions
- Enforce namespace-level authentication if SurrealDB supports it (future enhancement).
- For now, rely on tool config + MCP code to prevent cross-namespace writes.
- Add logging for `scope=all` queries for traceability.

## Testing Strategy
- Unit tests for namespace routing (mock config + assert queries hit expected namespace).
- Integration tests against a test SurrealDB instance with sample data.
- Regression tests to ensure existing functionality (thought logging, memory writes) still works.
