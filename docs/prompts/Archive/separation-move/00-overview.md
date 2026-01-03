# LegacyMind Persistent Memory Separation Plan

_Drafted by Codex — October 24, 2025_

## Intent

Split SurrealDB storage so every agent (Codex, Gemini, Claude, future members) keeps private context while sharing a single canonical knowledge base for federation-wide facts. All MCP tools will read an `agent` identity flag from configuration to decide where to store or query data.

## Target Layout

```
namespace: federation
  database: brain        # shared directives, SOPs, contact info
  database: policies     # optional, handoffs + shared TODOs (future)

namespace: codex
  database: brain        # Codex directives (graph)
  database: memories     # long-lived knowledge graph facts
  database: thoughts     # log of legacymind_think / photography_think outputs

namespace: gemini
  ...

namespace: claude
  ...
```

Every agent namespace mirrors the same trio (`brain`, `memories`, `thoughts`). Additional namespaces (e.g., `aftershoot`, `photography-mcp`) can be added later without touching the core tooling.

## Tooling Changes

1. **Config Identity Flag**
   - Add `"agent": "<name>"` (e.g., `"agent": "codex"`) to each MCP tool definition.
   - Tool code reads this flag and sets the SurrealDB namespace before executing queries.

2. **Scope Switch for Searches**
   - `legacymind_search` variants accept `scope=own|all`.
     - `own` (default): query caller namespace + `federation`.
     - `all`: fan out across all known namespaces.

3. **New `legacymind_brain` Tool**
   - CRUD interface keyed by `section` / `tag`.
   - Supports `namespace=federation` when editing shared directives.

4. **Retire Photography-Specific Tools**
   - Photography MCP uses the same Legacymind tools with its own `agent` flag.
   - Domain-specific behavior (Lightroom workflows, culling logs) lives in project repos/MCP modules, not SurrealDB tooling.

## Migration Steps (High-Level)

1. Snapshot existing SurrealDB databases.
2. Create namespaces/databases for each agent.
3. Transform existing brain file(s) into graph entries under the relevant namespace.
4. Migrate memories/thoughts to new locations (Scripts TBD).
5. Populate `federation.brain` with shared content (LegacyMind charter, SurrealMind endpoints, Sam bio, etc.).
6. Update MCP configs + tool implementations.
7. Validate read/write operations (own + federation + all).
8. Document ongoing maintenance (brain updates, cross-agent search etiquette).

Detailed migration checklist, schema, and scripts are documented in sibling files within this directory.
