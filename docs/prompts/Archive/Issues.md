# SurrealMind Issues Tracker

## Template Example

### ISSUE-TEMPLATE: [Brief Description]
**Date**: YYYY-MM-DD
**Found By**: [Federation Member Name]
**Severity**: CRITICAL | HIGH | MEDIUM | LOW
**Status**: OPEN | IN_PROGRESS | FIXED | WONT_FIX
**Component**: [e.g., inner_voice, embeddings, knowledge_graph]
**File(s)**: [Affected file paths]

**Description**:
[Detailed description of the issue]

**Reproduction Steps**:
1. [Step one]
2. [Step two]
3. [Observed result]

**Expected Behavior**:
[What should happen instead]

**Actual Behavior**:
[What actually happens]

**Proposed Fix**:
[Suggested solution if known]

**Notes**:
[Any additional context, workarounds, or references]

---

## Active Issues

### ISSUE-001: Inner Voice Entity Extraction Creating Fragment Entities
**Date**: 2025-09-17
**Found By**: CC
**Severity**: MEDIUM
**Status**: OPEN
**Component**: inner_voice
**File(s)**: src/tools/inner_voice.rs

**Description**:
The inner_voice tool's auto-extraction feature is creating low-quality entity fragments instead of meaningful concepts. When `auto_extract_to_kg: true` is set, it's extracting individual words like "The", "This", "Sources:" as entities rather than identifying actual concepts.

**Reproduction Steps**:
1. Call inner_voice with `auto_extract_to_kg: true`
2. Review pending entities with `memories_moderate action=review`
3. Observe single-word fragments in the approval queue

**Expected Behavior**:
Should extract meaningful entities like:
- Technical concepts (e.g., "MCP Server", "Embedding System")
- Project names (e.g., "SurrealMind", "LegacyMind")
- Tools and features (e.g., "inner_voice", "KG extraction")

**Actual Behavior**:
Extracts fragments like:
- "The"
- "This"
- "Sources:"
- "Following"
- "One"

**Proposed Fix**:
1. Implement minimum token length filter (e.g., > 2 tokens)
2. Add stop word filtering for common articles/prepositions
3. Use NER or more sophisticated extraction logic
4. Consider using LLM-based extraction with better prompting

**Notes**:
In moderation review on 2025-09-17, had to reject 23 out of 39 entities due to this issue. Also, all relationships had invalid entity IDs (showing as "NONE"), suggesting the extraction logic isn't properly linking entities.

---

### ISSUE-002: Knowledge Graph Relationships Missing Valid Entity IDs
**Date**: 2025-09-17
**Found By**: CC
**Severity**: HIGH
**Status**: OPEN
**Component**: inner_voice, knowledge_graph
**File(s)**: src/tools/inner_voice.rs, src/tools/knowledge_graph.rs

**Description**:
When inner_voice extracts relationships with auto_extract_to_kg, the relationships are created with source_id and target_id as "NONE", making them invalid and unapprovable.

**Reproduction Steps**:
1. Call inner_voice with `auto_extract_to_kg: true`
2. Review pending relationships with `memories_moderate action=review`
3. Attempt to approve relationships
4. Get error: "Could not resolve entity IDs for candidate edge"

**Expected Behavior**:
Relationships should have valid entity IDs that reference existing or newly created entities in the knowledge graph.

**Actual Behavior**:
All relationships show:
- source_id: "NONE"
- target_id: "NONE"

**Proposed Fix**:
1. Ensure entities are created and get valid IDs before creating relationships
2. Implement entity resolution to match entity names to existing IDs
3. Add validation before staging relationships to ensure valid IDs

**Notes**:
Had to reject all 16 relationships in the 2025-09-17 moderation queue due to this issue.

---

## Fixed Issues

[None yet]

---

## Issue Statistics
- Total Issues: 2
- Open: 2
- In Progress: 0
- Fixed: 0
- Won't Fix: 0

### By Severity
- CRITICAL: 0
- HIGH: 1
- MEDIUM: 1
- LOW: 0

### By Component
- inner_voice: 2
- knowledge_graph: 1