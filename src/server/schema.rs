use crate::server::SurrealMindServer;
use rmcp::ErrorData as McpError;
use tracing::info;

/// Emergency escape hatch for an already-known dimension mismatch. This is
/// intentionally narrow: it bypasses only dimension verification, never the
/// schema initialization itself. It must be available during
/// `SurrealMindServer::new()`, before `main` reaches its later preflight.
fn skip_dimension_check_value(value: Option<&str>) -> bool {
    matches!(value, Some("1")) || value.is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

pub fn skip_dimension_check_requested() -> bool {
    skip_dimension_check_value(std::env::var("SURR_SKIP_DIM_CHECK").ok().as_deref())
}

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
            DEFINE FIELD OVERWRITE embedding ON TABLE thoughts TYPE option<array<float>>;
            DEFINE FIELD injected_memories ON TABLE thoughts TYPE array<string>;
            DEFINE FIELD enriched_content ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD injection_scale ON TABLE thoughts TYPE int;
            DEFINE FIELD significance ON TABLE thoughts TYPE float;
            DEFINE FIELD access_count ON TABLE thoughts TYPE int;
            DEFINE FIELD last_accessed ON TABLE thoughts TYPE option<datetime>;
            DEFINE FIELD submode ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD framework_enhanced ON TABLE thoughts TYPE option<bool>;
            DEFINE FIELD OVERWRITE framework_analysis ON TABLE thoughts TYPE option<object> FLEXIBLE;
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
            -- Embedding status for graceful degradation (pending/complete/failed)
            DEFINE FIELD embedding_status ON TABLE thoughts TYPE option<string> DEFAULT "complete";
            DEFINE FIELD extracted_to_kg ON TABLE thoughts TYPE bool DEFAULT false;
            DEFINE FIELD extraction_batch_id ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD extracted_at ON TABLE thoughts TYPE option<datetime>;
            -- Mark fields for REMini correction system
            DEFINE FIELD marked_for ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD mark_type ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD mark_note ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD marked_at ON TABLE thoughts TYPE option<datetime>;
            DEFINE FIELD marked_by ON TABLE thoughts TYPE option<string>;
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
            DEFINE INDEX idx_thoughts_embedding_status ON TABLE thoughts FIELDS embedding_status;
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
            -- Mark fields for REMini correction system
            DEFINE FIELD marked_for ON TABLE kg_entities TYPE option<string>;
            DEFINE FIELD mark_type ON TABLE kg_entities TYPE option<string>;
            DEFINE FIELD mark_note ON TABLE kg_entities TYPE option<string>;
            DEFINE FIELD marked_at ON TABLE kg_entities TYPE option<datetime>;
            DEFINE FIELD marked_by ON TABLE kg_entities TYPE option<string>;
            -- Confidence decay fields
            DEFINE FIELD volatility ON TABLE kg_entities TYPE option<string> DEFAULT "medium";
            DEFINE FIELD last_refreshed ON TABLE kg_entities TYPE option<datetime>;
            DEFINE FIELD refresh_count ON TABLE kg_entities TYPE option<int> DEFAULT 0;
            DEFINE FIELD decay_confidence ON TABLE kg_entities TYPE option<float>;

            -- Journal thread fields
            DEFINE FIELD thread_status ON TABLE kg_entities TYPE option<string>;
            DEFINE INDEX idx_kge_entity_type_status ON TABLE kg_entities FIELDS entity_type, thread_status;

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
            -- Journal entry fields
            DEFINE FIELD author ON TABLE kg_observations TYPE option<string>;
            DEFINE FIELD observation_type ON TABLE kg_observations TYPE option<string>;
            DEFINE INDEX idx_kgo_author ON TABLE kg_observations FIELDS author;
            DEFINE INDEX idx_kgo_obs_type ON TABLE kg_observations FIELDS observation_type;
            -- Mark fields for REMini correction system
            DEFINE FIELD marked_for ON TABLE kg_observations TYPE option<string>;
            DEFINE FIELD mark_type ON TABLE kg_observations TYPE option<string>;
            DEFINE FIELD mark_note ON TABLE kg_observations TYPE option<string>;
            DEFINE FIELD marked_at ON TABLE kg_observations TYPE option<datetime>;
            DEFINE FIELD marked_by ON TABLE kg_observations TYPE option<string>;
            -- Confidence decay fields
            DEFINE FIELD volatility ON TABLE kg_observations TYPE option<string> DEFAULT "medium";
            DEFINE FIELD last_refreshed ON TABLE kg_observations TYPE option<datetime>;
            DEFINE FIELD refresh_count ON TABLE kg_observations TYPE option<int> DEFAULT 0;
            DEFINE FIELD decay_confidence ON TABLE kg_observations TYPE option<float>;

            -- CorrectionEvent table for REMini correction system
            DEFINE TABLE correction_events SCHEMAFULL;
            DEFINE FIELD timestamp ON TABLE correction_events TYPE datetime DEFAULT time::now();
            DEFINE FIELD target_id ON TABLE correction_events TYPE string;
            DEFINE FIELD target_table ON TABLE correction_events TYPE string;
            DEFINE FIELD previous_state ON TABLE correction_events TYPE object;
            DEFINE FIELD new_state ON TABLE correction_events TYPE object;
            DEFINE FIELD initiated_by ON TABLE correction_events TYPE string;
            DEFINE FIELD reasoning ON TABLE correction_events TYPE string;
            DEFINE FIELD sources ON TABLE correction_events TYPE array<string>;
            DEFINE FIELD verification_status ON TABLE correction_events TYPE string DEFAULT "auto_applied";
            DEFINE FIELD corrects_previous ON TABLE correction_events TYPE option<record<correction_events>>;
            DEFINE FIELD spawned_by ON TABLE correction_events TYPE option<record<correction_events>>;
            DEFINE INDEX idx_correction_events_target ON TABLE correction_events FIELDS target_id, target_table;
            DEFINE INDEX idx_correction_events_timestamp ON TABLE correction_events FIELDS timestamp;

            -- Agent exchange logging
            DEFINE TABLE agent_exchanges SCHEMAFULL;
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
            DEFINE TABLE tool_sessions SCHEMALESS;
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

            -- KG boundaries (rejected extractions with context)
            DEFINE TABLE kg_boundaries SCHEMALESS;
            DEFINE FIELD source_thought_id ON TABLE kg_boundaries TYPE string;
            DEFINE FIELD rejected ON TABLE kg_boundaries TYPE string;
            DEFINE FIELD reason ON TABLE kg_boundaries TYPE string;
            DEFINE FIELD context ON TABLE kg_boundaries TYPE option<string>;
            DEFINE FIELD confidence ON TABLE kg_boundaries TYPE option<float>;
            DEFINE FIELD extraction_batch_id ON TABLE kg_boundaries TYPE option<string>;
            DEFINE FIELD extracted_at ON TABLE kg_boundaries TYPE option<datetime>;
            DEFINE FIELD extraction_prompt_version ON TABLE kg_boundaries TYPE option<string>;
            DEFINE INDEX idx_kgbnd_created ON TABLE kg_boundaries FIELDS created_at;
            DEFINE INDEX idx_kgbnd_thought ON TABLE kg_boundaries FIELDS source_thought_id;
            DEFINE INDEX idx_kgbnd_batch ON TABLE kg_boundaries FIELDS extraction_batch_id;
        "#
        );

        // NOTE on `.check()`: measured live during this pass, adding
        // `.check()` to this call breaks re-initialization of an
        // ALREADY-INITIALIZED schema. None of these DDL statements carry
        // OVERWRITE or IF NOT EXISTS, so SurrealDB 3.1.2 errors on every
        // redefinition (even byte-identical ones) with e.g. "The table
        // 'thoughts' already exists" — and `initialize_schema()` runs on
        // every `SurrealMindServer::new()`, i.e. every server restart
        // against a live namespace. Production currently depends on
        // `.query().await` NOT inspecting per-statement results to make
        // repeated startup idempotent. `.check()` would surface that first
        // "already exists" error (not the dimension-drift error this was
        // meant to catch) and fail every restart. Making the DDL itself
        // idempotent (OVERWRITE/IF NOT EXISTS) is the correct fix for that,
        // and is explicitly out of scope for this patch because rebuilding a
        // live HNSW index is a deliberate operational migration, not a
        // restart-time side effect.
        // So this call is left as-is; the visibility this task actually
        // asked for (a stale-dimension thoughts_embedding_idx going
        // unnoticed) is provided by the explicit post-init verification
        // below instead, which does not touch DDL error-swallowing at all.
        self.db.query(schema_sql).await.map_err(|e| McpError {
            code: rmcp::model::ErrorCode::INTERNAL_ERROR,
            message: format!("Schema init failed: {}", e).into(),
            data: None,
        })?;

        if skip_dimension_check_requested() {
            info!(
                "SURR_SKIP_DIM_CHECK is set; bypassing thoughts_embedding_idx dimension verification"
            );
        } else {
            self.verify_embedding_index_dimension(dim).await?;
        }

        Ok(())
    }

    /// Read back `thoughts_embedding_idx`'s live DIMENSION via `INFO FOR
    /// TABLE thoughts` and compare it against the currently configured
    /// embedder dimension. This is the visibility fix for the scenario the
    /// postdeploy review named directly: a `DEFINE INDEX ... HNSW DIMENSION
    /// {dim}` redefinition at a different dimension silently no-ops (per the
    /// note above) instead of erroring, so the index keeps serving the OLD
    /// dimension while every other signal claims schema init succeeded.
    /// Deliberately read-only: it reports the drift as an error rather than
    /// auto-remediating (rebuilding a live HNSW index is a real design
    /// decision, not a restart-time side effect).
    async fn verify_embedding_index_dimension(
        &self,
        expected_dim: usize,
    ) -> std::result::Result<(), McpError> {
        let info: Vec<serde_json::Value> = self
            .db
            .query("INFO FOR TABLE thoughts")
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to read back thoughts table info: {}", e).into(),
                data: None,
            })?
            .take(0)
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to parse thoughts table info: {}", e).into(),
                data: None,
            })?;

        let index_def = info
            .first()
            .and_then(|v| v.get("indexes"))
            .and_then(|v| v.get("thoughts_embedding_idx"))
            .and_then(|v| v.as_str());

        let Some(index_def) = index_def else {
            // No index at all is a startup-order anomaly (the DEFINE INDEX
            // statement above should have created it), not a dimension
            // mismatch. Leave detecting that class of failure to the
            // existing health_check_indexes maintenance tool rather than
            // failing server startup here.
            return Ok(());
        };

        // `index_def` is raw DDL text, e.g.
        // "DEFINE INDEX thoughts_embedding_idx ON thoughts FIELDS embedding HNSW DIMENSION 1536 ...".
        // Extract the integer following "DIMENSION".
        let live_dim = index_def
            .split_whitespace()
            .collect::<Vec<_>>()
            .windows(2)
            .find(|w| w[0].eq_ignore_ascii_case("DIMENSION"))
            .and_then(|w| {
                w[1].trim_matches(|c: char| !c.is_ascii_digit())
                    .parse::<usize>()
                    .ok()
            });

        match live_dim {
            Some(live_dim) if live_dim != expected_dim => Err(McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!(
                    "thoughts_embedding_idx is defined at DIMENSION {} but the configured embedder expects {}. \
                     The DEFINE INDEX statement in initialize_schema() does not carry OVERWRITE, so a dimension \
                     change silently no-ops instead of rebuilding the index (see the note in initialize_schema()). \
                     This requires a deliberate index rebuild, not automatic remediation: {:?}",
                    live_dim, expected_dim, index_def
                )
                .into(),
                data: None,
            }),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::skip_dimension_check_value;

    #[test]
    fn skip_dimension_check_flag_accepts_only_documented_truthy_values() {
        assert!(skip_dimension_check_value(Some("1")));
        assert!(skip_dimension_check_value(Some("true")));
        assert!(skip_dimension_check_value(Some("TRUE")));
        assert!(!skip_dimension_check_value(None));
        assert!(!skip_dimension_check_value(Some("0")));
        assert!(!skip_dimension_check_value(Some("false")));
    }
}
