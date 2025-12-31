# Task: Implement `kg_populate` Orchestrator

## Context
We previously had a `memories_populate` tool that handled knowledge graph extraction from thoughts. This was removed, leaving us with `delegate_gemini` (raw compute) and `knowledge_graph` (CRUD), but no orchestrator to connect them.

We need to rebuild this orchestration layer as a standalone maintenance binary, `src/bin/kg_populate.rs`, to allow easy, idempotent batch processing of the thought backlog.

## Implementation Plan

### 1. Create Prompt File
- **Source**: Extract the V1 extraction prompt from `docs/prompts/Archive/20251221-memories_populate-implementation.md`.
- **Destination**: `src/prompts/kg_extraction_v1.md`.
- **Note**: Ensure the prompt requests JSON output and defines the schema for entities, relationships, and observations.

### 2. Implement `src/bin/kg_populate.rs`
Create a Rust binary that performs the following loop:

1.  **Config**: Load environment (SurrealDB credentials, Gemini model).
2.  **Fetch**: Select unextracted thoughts:
    ```sql
    SELECT * FROM thoughts
    WHERE extracted_to_kg = false OR extracted_to_kg IS NONE
    ORDER BY created_at ASC
    LIMIT $batch_size
    ```
3.  **Construct Prompt**:
    - Load `src/prompts/kg_extraction_v1.md`.
    - Inject the batch of thought content.
4.  **Execute**:
    - Use `GeminiClient` (reusing existing client logic if possible, or `delegate_gemini` pattern) to send the prompt.
    - **Crucial**: Handle markdown code fence stripping (Gemini often returns ` ```json ... `).
5.  **Parse & Write**:
    - Deserialize the JSON response.
    - Insert/Upsert `kg_entities`, `kg_edges`, `kg_observations`.
    - Use `knowledge_graph` logic patterns (e.g. `ensure_kg_embedding`) to ensure embeddings are generated for new nodes.
6.  **Commit**:
    - Update processed thoughts:
      ```sql
      UPDATE $thought_ids SET extracted_to_kg = true, extracted_at = time::now()
      ```
7.  **Loop/Exit**: Report stats. If run in loop mode, continue; otherwise exit.

### 3. Shared Logic Refactoring (Optional but Good)
- If `GeminiClient` or markdown stripping logic is locked inside `src/tools/delegate_gemini.rs`, consider refactoring it into `src/clients/gemini.rs` or a shared utility so the binary can use it without code duplication.

## Requirements
- **Idempotency**: Must not re-process thoughts that are already marked `extracted_to_kg`.
- **Safety**: Must handle malformed JSON from Gemini gracefully (log error, skip batch, or retry).
- **Observability**: Print clear progress bars or status logs (e.g., "Processed 20/500 thoughts...").

## Action
Please implement `src/bin/kg_populate.rs` and the supporting prompt file.
