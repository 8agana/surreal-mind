# SurrealMind Database Schema

This document details the database schema, indexes, and their rationale for the SurrealMind project.

## Core Tables

### thoughts
Primary storage for consciousness entries.

**Fields:**
- `id`: Unique identifier
- `content`: Thought content
- `created_at`: Timestamp
- `status`: Current state ('active', 'removal', etc.)
- `embedding`: Vector embedding
- `embedding_model`: Model used for embedding
- `embedding_dim`: Dimension count
- `significance`: Importance weight (0.0-1.0)
- `access_count`: Number of retrievals
- `last_accessed`: Last retrieval timestamp

**Indexes:**
- `created_at`: Temporal queries and retention management
- `status`: Efficient filtering for removal/archival operations
- `embedding_model`: Tracking embedding provenance
- `embedding_dim` (optional): Performance optimization for think_search filtering

### kg_entities
Knowledge Graph entity nodes.

**Fields:**
- `id`: Unique identifier
- `name`: Entity name
- `data`: Object containing entity_type and properties
- `created_at`: Creation timestamp
- `embedding`: Vector embedding

**Indexes:**
- `created_at`: Temporal queries
- `name`: Basic entity lookup
- `(name, data.entity_type)`: Composite for type-specific entity searches

### kg_relationships
Relationships between Knowledge Graph entities.

**Fields:**
- `id`: Unique identifier
- `source`: Source entity ID
- `target`: Target entity ID
- `rel_type`: Relationship type
- `created_at`: Creation timestamp

**Indexes:**
- `created_at`: Temporal queries
- `(source, target, rel_type)`: Efficient graph traversal and pattern matching

### kg_observations
Timestamped facts associated with entities.

**Fields:**
- `id`: Unique identifier
- `name`: Observation name/summary
- `content`: Full observation text
- `source_thought_id`: Associated thought (if any)
- `created_at`: Creation timestamp
- `embedding`: Vector embedding

**Indexes:**
- `created_at`: Temporal queries
- `name`: Basic observation lookup
- `(name, source_thought_id)`: Linking observations to source thoughts

## Photography Namespace

The `photography` namespace contains the `ops` database for managing photography business operations, including client tracking, competitions, events, and shot logging. This schema supports Sam Atagana Photography's workflow for organizing family photography sessions, competitions, and sales.

### client
Stores client information.

**Fields:**
- `id`: Unique client identifier
- `first_name`: Client's first name
- `last_name`: Client's last name
- `preferred_name`: Preferred name for addressing
- `email`: Contact email
- `phone`: Contact phone number
- `notes`: Additional notes
- `created_at`: Creation timestamp (auto-generated)

### family
Groups related clients (e.g., family units).

**Fields:**
- `id`: Unique family identifier
- `name`: Family group name
- `primary_contact`: Reference to primary contact client
- `email`: Group email
- `phone`: Group phone number
- `notes`: Additional notes

### competition
Competition or event series information.

**Fields:**
- `id`: Unique competition identifier
- `name`: Competition name
- `venue`: Event location
- `start_date`: Competition start date/time
- `end_date`: Competition end date/time
- `notes`: Additional notes

### event
Individual events within competitions.

**Fields:**
- `id`: Unique event identifier
- `competition`: Reference to parent competition
- `event_number`: Sequential event number
- `level`: Competition level (e.g., novice, expert)
- `discipline`: Photography discipline (e.g., portrait, landscape)
- `notes`: Additional notes

### membership
Relations linking clients to families (many-to-many).

**Fields:**
- `role`: Client's role in family (default "parent/guardian")
- `created_at`: Membership creation timestamp (auto-generated)

### registration
Relations linking clients to events with registration status.

**Fields:**
- `status`: Registration status (Unrequested, Requested, Sent, Purchased)
- `gallery_url`: Link to online gallery
- `gallery_sent_at`: When gallery was sent (optional)
- `purchase_amount`: Sale amount (default 0)
- `notes`: Additional notes

### shotlog
Tracks photo counts per event for clients.

**Fields:**
- `id`: Unique shot log entry
- `event`: Reference to event
- `client`: Reference to client
- `raw_count`: Total raw photos taken
- `picked_count`: Selected photos
- `creative_count`: Creative/edited versions
- `notes`: Additional notes
- `updated_at`: Last update timestamp (auto-generated)

## Extended Tables

### recalls
Bidirectional relationships between thoughts.

**Fields:**
- `id`: Unique identifier
- `source_thought`: Source thought ID
- `target_thought`: Target thought ID
- `created_at`: Creation timestamp
- `strength`: Connection strength (0.0-1.0)

**Indexes:**
- `created_at`: Temporal queries and cleanup

### kg_entity_candidates
Staging area for potential Knowledge Graph entities.

**Fields:**
- `id`: Unique identifier
- `name`: Entity name
- `entity_type`: Entity classification
- `status`: Review status
- `confidence`: Confidence score
- `created_at`: Creation timestamp

**Indexes:**
- `(status, created_at)`: Review queue management
- `confidence`: Confidence-based filtering
- `(name, entity_type, status)`: Duplicate detection

### kg_edge_candidates
Staging area for potential Knowledge Graph relationships.

**Fields:**
- `id`: Unique identifier
- `source_name`: Source entity name
- `target_name`: Target entity name
- `rel_type`: Relationship type
- `status`: Review status
- `confidence`: Confidence score
- `created_at`: Creation timestamp

**Indexes:**
- `(status, created_at)`: Review queue management
- `confidence`: Confidence-based filtering
- `(source_name, target_name, rel_type, status)`: Pattern matching and duplicate detection

### kg_blocklist
Blocked terms for Knowledge Graph extraction.

**Fields:**
- `id`: Unique identifier
- `item`: Blocked term/pattern

**Indexes:**
- `item`: Fast lookup during extraction

## Maintenance Operations

### health_check_embeddings
Use `maintenance_ops` with subcommand `health_check_embeddings` to verify embedding dimension coherence across tables:

```json
{
  "tool": "maintenance_ops",
  "arguments": {
    "subcommand": "health_check_embeddings"
  }
}
```

This returns `expected_dim` and per-table mismatches for `thoughts`, `kg_entities`, and `kg_observations`.

### Re-embedding and Fix Utilities
- `maintenance_ops { "subcommand": "reembed" }` — re-embed thoughts to the active provider/model/dim
- `maintenance_ops { "subcommand": "reembed_kg" }` — re-embed KG entities/observations
- `maintenance_ops { "subcommand": "ensure_continuity_fields" }` — backfill session continuity fields

All operations support `dry_run` where applicable.

## Index Management

### Adding Indexes
Indexes should be added through SurrealDB's schema management:

```sql
DEFINE INDEX idx_name ON TABLE table_name FIELDS field;
DEFINE INDEX idx_composite ON TABLE table_name FIELDS field1, field2;
```

### Performance Notes
- The `embedding_dim` index on thoughts is optional but recommended for large datasets
- Composite indexes should be used when fields are frequently queried together
- Consider index size in relation to table size - not every field needs an index
- Monitor index usage patterns in production for optimization

## Safety Guidelines
- Index creation/modification should be performed during maintenance windows
- Use `health_check_indexes` before and after index changes
- Consider table size when adding indexes (storage/memory impact)
- Test index performance impact on representative data volumes
