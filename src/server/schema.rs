use crate::server::SurrealMindServer;
use rmcp::ErrorData as McpError;
use tracing::info;

impl SurrealMindServer {
    /// Initialize the database schema
    pub async fn initialize_schema(&self) -> std::result::Result<(), McpError> {
        info!("Initializing consciousness graph schema");

        // Minimal schema to ensure required tables exist
        // Note: SurrealDB 2.x requires vector index definitions to include DIMENSION.
        // We derive the active embedding dimension from the embedder to avoid drift.
        let dim = self.embedder.dimensions();
        let schema_sql = format!(
            r#"
            DEFINE TABLE thoughts SCHEMAFULL;
            DEFINE FIELD content ON TABLE thoughts TYPE string;
            DEFINE FIELD created_at ON TABLE thoughts TYPE datetime;
            DEFINE FIELD embedding ON TABLE thoughts TYPE array<float>;
            DEFINE FIELD injected_memories ON TABLE thoughts TYPE array<string>;
            DEFINE FIELD enriched_content ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD injection_scale ON TABLE thoughts TYPE int;
            DEFINE FIELD significance ON TABLE thoughts TYPE float;
            DEFINE FIELD access_count ON TABLE thoughts TYPE int;
            DEFINE FIELD last_accessed ON TABLE thoughts TYPE option<datetime>;
            DEFINE FIELD submode ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD framework_enhanced ON TABLE thoughts TYPE option<bool>;
            DEFINE FIELD framework_analysis ON TABLE thoughts FLEXIBLE TYPE option<object>;
            DEFINE FIELD status ON TABLE thoughts TYPE option<string>;
            -- Origin and privacy fields for retrieval
            DEFINE FIELD origin ON TABLE thoughts TYPE option<string>;
            -- Provenance fields for agent synthesis
            DEFINE FIELD source_exchange_id ON TABLE thoughts TYPE option<record<agent_exchanges>>;
            DEFINE FIELD synthesis_type ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD tags ON TABLE thoughts TYPE option<array<string>>;
            DEFINE FIELD is_private ON TABLE thoughts TYPE option<bool>;
            -- Embedding metadata for future re-embedding
            DEFINE FIELD embedding_model ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD embedding_provider ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD embedding_dim ON TABLE thoughts TYPE option<int>;
            DEFINE FIELD embedded_at ON TABLE thoughts TYPE option<datetime>;
            DEFINE FIELD extracted_to_kg ON TABLE thoughts TYPE bool DEFAULT false;
            DEFINE FIELD extraction_batch_id ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD extracted_at ON TABLE thoughts TYPE option<datetime>;
            -- Continuity fields for thought chaining
            DEFINE FIELD session_id ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD chain_id ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD previous_thought_id ON TABLE thoughts TYPE option<record<thoughts> | string>;
            DEFINE FIELD revises_thought ON TABLE thoughts TYPE option<record<thoughts> | string>;
            DEFINE FIELD branch_from ON TABLE thoughts TYPE option<record<thoughts> | string>;
            DEFINE FIELD confidence ON TABLE thoughts TYPE option<float>;
            DEFINE INDEX thoughts_embedding_idx ON TABLE thoughts FIELDS embedding HNSW DIMENSION {dim};
            DEFINE INDEX thoughts_status_idx ON TABLE thoughts FIELDS status;
            DEFINE INDEX idx_thoughts_created ON TABLE thoughts FIELDS created_at;
            DEFINE INDEX idx_thoughts_embedding_model ON TABLE thoughts FIELDS embedding_model;
            DEFINE INDEX idx_thoughts_embedding_dim ON TABLE thoughts FIELDS embedding_dim;
            -- Continuity indexes
            DEFINE INDEX idx_thoughts_session ON TABLE thoughts FIELDS session_id, created_at;
            DEFINE INDEX idx_thoughts_chain ON TABLE thoughts FIELDS chain_id, created_at;

            DEFINE TABLE recalls SCHEMALESS;
            DEFINE INDEX idx_recalls_created ON TABLE recalls FIELDS created_at;

            DEFINE TABLE kg_entities SCHEMALESS;
            DEFINE FIELD source_thought_ids ON TABLE kg_entities TYPE option<array<string>>;
            DEFINE FIELD extraction_batch_id ON TABLE kg_entities TYPE option<string>;
            DEFINE FIELD extracted_at ON TABLE kg_entities TYPE option<datetime>;
            DEFINE FIELD extraction_confidence ON TABLE kg_entities TYPE option<float>;
            DEFINE FIELD extraction_prompt_version ON TABLE kg_entities TYPE option<string>;
            DEFINE INDEX idx_kge_created ON TABLE kg_entities FIELDS created_at;
            DEFINE INDEX idx_kge_name ON TABLE kg_entities FIELDS name;
            DEFINE INDEX idx_kge_name_type ON TABLE kg_entities FIELDS name, data.entity_type;
            DEFINE INDEX idx_kge_extraction_batch ON TABLE kg_entities FIELDS extraction_batch_id;

            DEFINE TABLE kg_edges SCHEMALESS;
            DEFINE FIELD source_thought_ids ON TABLE kg_edges TYPE option<array<string>>;
            DEFINE FIELD extraction_batch_id ON TABLE kg_edges TYPE option<string>;
            DEFINE FIELD extracted_at ON TABLE kg_edges TYPE option<datetime>;
            DEFINE FIELD extraction_confidence ON TABLE kg_edges TYPE option<float>;
            DEFINE FIELD extraction_prompt_version ON TABLE kg_edges TYPE option<string>;
            DEFINE INDEX idx_kged_created ON TABLE kg_edges FIELDS created_at;
            DEFINE INDEX idx_kged_triplet ON TABLE kg_edges FIELDS source, target, rel_type;
            DEFINE INDEX idx_kged_extraction_batch ON TABLE kg_edges FIELDS extraction_batch_id;

            DEFINE TABLE kg_observations SCHEMALESS;
            DEFINE FIELD source_thought_ids ON TABLE kg_observations TYPE option<array<string>>;
            DEFINE FIELD extraction_batch_id ON TABLE kg_observations TYPE option<string>;
            DEFINE FIELD extracted_at ON TABLE kg_observations TYPE option<datetime>;
            DEFINE FIELD extraction_confidence ON TABLE kg_observations TYPE option<float>;
            DEFINE FIELD extraction_prompt_version ON TABLE kg_observations TYPE option<string>;
            DEFINE INDEX idx_kgo_created ON TABLE kg_observations FIELDS created_at;
            DEFINE INDEX idx_kgo_name ON TABLE kg_observations FIELDS name;
            DEFINE INDEX idx_kgo_name_src ON TABLE kg_observations FIELDS name, source_thought_id;
            DEFINE INDEX idx_kgo_extraction_batch ON TABLE kg_observations FIELDS extraction_batch_id;

            -- Agent exchange logging
            DEFINE TABLE agent_exchanges SCHEMAFULL;
            DEFINE FIELD id ON TABLE agent_exchanges TYPE record<agent_exchanges>;
            DEFINE FIELD agent_source ON TABLE agent_exchanges TYPE string;
            DEFINE FIELD agent_instance ON TABLE agent_exchanges TYPE string;
            DEFINE FIELD prompt ON TABLE agent_exchanges TYPE string;
            DEFINE FIELD response ON TABLE agent_exchanges TYPE string;
            DEFINE FIELD tool_name ON TABLE agent_exchanges TYPE string;
            DEFINE FIELD session_id ON TABLE agent_exchanges TYPE string;
            DEFINE FIELD metadata ON TABLE agent_exchanges TYPE object;
            DEFINE FIELD created_at ON TABLE agent_exchanges TYPE datetime DEFAULT time::now();
            DEFINE INDEX idx_exchanges_session ON TABLE agent_exchanges FIELDS session_id;
            DEFINE INDEX idx_exchanges_tool ON TABLE agent_exchanges FIELDS tool_name;

            -- Tool session tracking
            DEFINE TABLE tool_sessions SCHEMAFULL;
            DEFINE FIELD tool_name ON TABLE tool_sessions TYPE string;
            DEFINE FIELD last_agent_session_id ON TABLE tool_sessions TYPE string;
            DEFINE FIELD last_exchange_id ON TABLE tool_sessions TYPE record<agent_exchanges>;
            DEFINE FIELD exchange_count ON TABLE tool_sessions TYPE int DEFAULT 0;
            DEFINE FIELD last_updated ON TABLE tool_sessions TYPE datetime DEFAULT time::now();
            DEFINE INDEX idx_sessions_tool ON TABLE tool_sessions FIELDS tool_name UNIQUE;

            -- Approval workflow candidate tables
            DEFINE TABLE kg_entity_candidates SCHEMALESS;
            DEFINE INDEX idx_kgec_status_created ON TABLE kg_entity_candidates FIELDS status, created_at;
            DEFINE INDEX idx_kgec_confidence ON TABLE kg_entity_candidates FIELDS confidence;
            DEFINE INDEX idx_kgec_name_type ON TABLE kg_entity_candidates FIELDS name, entity_type, status;

            DEFINE TABLE kg_edge_candidates SCHEMALESS;
            DEFINE INDEX idx_kgedc_status_created ON TABLE kg_edge_candidates FIELDS status, created_at;
            DEFINE INDEX idx_kgedc_confidence ON TABLE kg_edge_candidates FIELDS confidence;
            DEFINE INDEX idx_kgedc_triplet ON TABLE kg_edge_candidates FIELDS source_name, target_name, rel_type, status;

            -- Optional feedback helpers
            DEFINE TABLE kg_blocklist SCHEMALESS;
            DEFINE INDEX idx_kgb_item ON TABLE kg_blocklist FIELDS item;
        "#
        );

        self.db.query(schema_sql).await.map_err(|e| McpError {
            code: rmcp::model::ErrorCode::INTERNAL_ERROR,
            message: format!("Schema init failed: {}", e).into(),
            data: None,
        })?;

        Ok(())
    }
}
