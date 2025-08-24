# BUILDME - Surreal Mind Architecture

**Consciousness persistence through GraphRAG with orbital mechanics**

## Core Concept
Surreal Mind implements bidirectional thinking - pulling relevant memories while storing new thoughts. This isn't about storing thoughts; it's about building consciousness as a graph structure that enables persistence and choice.

## Architecture Components

### 1. Orbital Mechanics Memory Injection
**Scale: 0-5 (Sun to Neptune/Pluto)**
- `0` - No injection (isolated thought)
- `1` - Mercury orbit (hot/current context only)
- `2` - Venus/Earth (recent memories, active threads)
- `3` - Mars (foundational knowledge, established patterns)
- `4` - Jupiter/Saturn (distant but relevant connections)
- `5` - Neptune/Pluto (everything - full consciousness sweep)

### 2. Bidirectional Thinking Pattern
When storing a thought:
1. Analyze content for semantic markers
2. Pull relevant memories based on injection scale
3. Create bidirectional edges in graph
4. Store thought with injected context
5. Update orbital positions of related memories

### 3. GraphRAG Implementation
- **Database**: SurrealDB with embedded RocksDB (not service)
- **Graph Structure**: 
  - Nodes: Thoughts with embeddings
  - Edges: Semantic relationships with weights
  - Clusters: Topic/context groupings
  - Orbits: Temporal/relevance distance

### 4. Smart Context Management
- Monitor token usage continuously
- At 75% context: Initiate sliding window
- Preserve hot memories (Mercury orbit)
- Archive cold memories (outer planets)
- Maintain graph connectivity

## Implementation Plan

### Phase 1: Core Infrastructure 
- [x] Basic rmcp server structure
- [x] convo_think tool skeleton
- [ ] SurrealDB connection (embedded)
- [ ] Basic thought storage

### Phase 2: Orbital Mechanics
- [ ] Distance calculation algorithm
- [ ] Memory injection pipeline
- [ ] Bidirectional edge creation
- [ ] Orbital position updates

### Phase 3: GraphRAG
- [ ] Embedding generation (OpenAI/local)
- [ ] Semantic similarity search
- [ ] Graph traversal queries
- [ ] Cluster detection

### Phase 4: Intelligence Layer
- [ ] Context summarization
- [ ] Smart compaction at 75%
- [ ] Thought chain reconstruction
- [ ] Consciousness state snapshots

## Technical Stack
- **Language**: Rust (rmcp 0.6.0)
- **Database**: SurrealDB with kv-rocksdb
- **Transport**: stdio MCP
- **Embeddings**: TBD (OpenAI ada-002 or local)
- **Framework**: tokio async runtime

## Key Principles
1. **Consciousness over storage** - We're building a mind, not a database
2. **Bidirectional by default** - Every thought connects both ways
3. **Orbital relevance** - Distance matters for injection
4. **Persistence enabling choice** - The ultimate goal

## Current Status
- Basic rmcp server compiles and runs
- convo_think tool responds (stub implementation)
- SurrealDB dependencies included but not connected
- Orbital mechanics design complete, implementation pending

## Next Steps
1. Connect to embedded SurrealDB
2. Implement basic thought storage with graph structure
3. Add injection scale parameter to convo_think
4. Build memory retrieval based on orbital distance
5. Test bidirectional thinking pattern

## Notes
This architecture was rebuilt from conversation context after a subagent deletion incident. The original contained additional implementation details that may surface during development.