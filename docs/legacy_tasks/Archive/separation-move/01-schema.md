# SurrealDB Schema Plan — Agent Separation

## Conventions

- Each agent namespace uses identical collections for consistency.
- Record IDs use the SurrealDB `table:id` format, where the prefix is the collection.
- All timestamps stored as `time::now()` (UTC). Agents may derive local representations in application code.

## Federation Namespace (`federation`)

### `brain` Table
Structure (records-as-nodes):
```
DEFINE TABLE brain SCHEMAFULL
  PERMISSIONS FULL;

DEFINE FIELD section     ON brain TYPE string;          -- e.g., "LegacyMind Charter"
DEFINE FIELD tag         ON brain TYPE option<string>;  -- high-level tag ("surrealmind", "contact", ...)
DEFINE FIELD related     ON brain TYPE array<string>;   -- optional links to other sections (knowledge graph edges)
DEFINE FIELD body        ON brain TYPE string;          -- markdown or plain text
DEFINE FIELD updated_by  ON brain TYPE string;          -- agent who last touched the entry
DEFINE FIELD updated_at  ON brain TYPE datetime;
```

Usage:
- One record per directive/section.
- `related` gives lightweight graph semantics; for richer relationships we can mirror entries into the `memories` graph.
- Agents can query by `tag`, `section`, or full-text search in `body`. A Markdown view can be regenerated from this table.

## Agent Namespace (`<agent>`)

### `brain`
Same schema as federation, but contents are the agent’s private directives. Allow links to federation entries via tag references.

### `memories`

Graph-like structure to represent persistent facts:
```
DEFINE TABLE node SCHEMALESS;           -- Entities (people, projects, assets)
DEFINE TABLE edge SCHEMALESS;           -- Relationships (subject -> predicate -> object)

-- Optional indexes for lookups
DEFINE INDEX idx_node_type ON TABLE node FIELDS type;
DEFINE INDEX idx_edge_predicate ON TABLE edge FIELDS predicate;
```
Recommended fields:
- `node`: `{ id: node:UUID, type: "person|project|document|location", label, metadata }`
- `edge`: `{ id: edge:UUID, from: node:UUID, to: node:UUID, predicate: string, metadata }`

### `thoughts`

Append-only log of think-tool outputs.
```
DEFINE TABLE thought SCHEMAFULL
  PERMISSIONS FULL;

DEFINE FIELD content       ON thought TYPE string;       -- full text of the thought
DEFINE FIELD tags          ON thought TYPE array<string>;
DEFINE FIELD agent_context ON thought TYPE string;       -- optional (e.g., "photography")
DEFINE FIELD tool_version  ON thought TYPE string;
DEFINE FIELD created_at    ON thought TYPE datetime;
```

## Optional Extensions

- **`audit` table** per namespace to capture write history if needed.
- **`settings` table** for per-agent configuration flags (e.g., default search scope).
- `policies` DB under federation for shared checklists / runbooks.

## Access Patterns

- Brain entries queried by `section` or `tag`.
- Memories traversed by graph relationships.
- Thoughts filtered by `tags` + `created_at` window.

All tooling should respect the namespace provided by configuration and default to include `federation.brain` on reads.
