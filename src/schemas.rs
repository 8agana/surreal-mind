use serde_json::{Map, Value, json};
use std::sync::Arc;

pub fn convo_think_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "content": {"type": "string"},
            "injection_scale": {"type": ["integer", "string"]},
            "tags": {"type": "array", "items": {"type": "string"}},
            "significance": {"type": ["number", "string"]},
            "verbose_analysis": {"type": "boolean"}
        },
        "required": ["content"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn legacymind_think_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "content": {"type": "string"},
            "hint": {"type": "string", "enum": ["debug", "build", "plan", "stuck", "question", "conclude"]},
            "injection_scale": {"type": ["integer", "string"]},
            "tags": {"type": "array", "items": {"type": "string"}},
            "significance": {"type": ["number", "string"]},
            "verbose_analysis": {"type": "boolean"},
            "session_id": {"type": "string"},
            "chain_id": {"type": "string"},
            "previous_thought_id": {"type": "string"},
            "revises_thought": {"type": "string"},
            "branch_from": {"type": "string"},
            "confidence": {"type": "number", "minimum": 0.0, "maximum": 1.0},
            "hypothesis": {"type": "string"},
            "needs_verification": {"type": "boolean"},
            "verify_top_k": {"type": "integer", "minimum": 1, "maximum": 500},
            "min_similarity": {"type": "number", "minimum": 0.0, "maximum": 1.0},
            "evidence_limit": {"type": "integer", "minimum": 1, "maximum": 25},
            "contradiction_patterns": {"type": "array", "items": {"type": "string"}}
        },
        "required": ["content"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn search_thoughts_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "content": {"type": "string"},
            "top_k": {"type": ["integer", "number", "string"], "minimum": 1, "maximum": 50},
            "offset": {"type": ["integer", "number", "string"], "minimum": 0},
            "sim_thresh": {"type": "number", "minimum": 0.0, "maximum": 1.0},
            "min_significance": {"type": "number", "minimum": 0.0, "maximum": 1.0},
            "expand_graph": {"type": "boolean"},
            "graph_depth": {"type": ["integer", "number", "string"], "minimum": 0, "maximum": 3},
            "graph_boost": {"type": "number"},
            "min_edge_strength": {"type": "number"},
            "sort_by": {"type": "string", "enum": ["score", "similarity", "recency", "significance"], "default": "score"}
        },
        "required": ["content"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn kg_create_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "kind": {"type": "string", "enum": ["entity", "relationship", "observation"], "default": "entity"},
            "data": {"type": "object"},
            "upsert": {"type": "boolean", "default": true},
            "source_thought_id": {"type": "string"},
            "confidence": {"type": "number", "minimum": 0.0, "maximum": 1.0}
        },
        "required": ["kind", "data"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn kg_search_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "query": {"type": "object"},
            "target": {"type": "string", "enum": ["entity", "relationship", "observation", "mixed"], "default": "mixed"},
            "top_k": {"type": ["integer", "number", "string"], "minimum": 1, "maximum": 50}
        }
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn kg_review_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "target": {"type": "string", "enum": ["entity", "relationship", "mixed"], "default": "mixed"},
            "status": {"type": "string", "enum": ["pending", "approved", "rejected", "auto_approved"], "default": "pending"},
            "min_conf": {"type": ["number", "string"], "minimum": 0.0, "maximum": 1.0},
            "limit": {"type": ["integer", "number", "string"], "minimum": 1, "maximum": 200, "default": 50},
            "offset": {"type": ["integer", "number", "string"], "minimum": 0, "default": 0},
            "query": {"type": "object"}
        }
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn kg_decide_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "items": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "kind": {"type": "string", "enum": ["entity", "relationship"]},
                        "decision": {"type": "string", "enum": ["approve", "reject"]},
                        "feedback": {"type": "string"},
                        "canonical_id": {"type": "string"}
                    },
                    "required": ["id", "kind", "decision"]
                }
            }
        },
        "required": ["items"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn kg_moderate_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "action": {"type": "string", "enum": ["review", "decide", "review_and_decide"], "default": "review"},
            "target": {"type": "string", "enum": ["entity", "relationship", "mixed"], "default": "mixed"},
            "status": {"type": "string", "enum": ["pending", "approved", "rejected", "auto_approved"], "default": "pending"},
            "min_conf": {"type": ["number", "string"], "minimum": 0.0, "maximum": 1.0},
            "limit": {"type": ["integer", "number", "string"], "minimum": 1, "maximum": 200, "default": 50},
            "offset": {"type": ["integer", "number", "string"], "minimum": 0, "default": 0},
            "cursor": {"type": "string"},
            "query": {"type": "object"},
            "items": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "kind": {"type": "string", "enum": ["entity", "relationship"]},
                        "decision": {"type": "string", "enum": ["approve", "reject", "alias"]},
                        "feedback": {"type": "string"},
                        "canonical_id": {"type": "string"}
                    },
                    "required": ["id", "kind", "decision"]
                }
            },
            "dry_run": {"type": "boolean", "default": false}
        }
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn detailed_help_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "tool": {"type": "string", "enum": [
                // Canonical tool names only
                "think_convo", "think_plan", "think_debug", "think_build", "think_stuck",
                "memories_create", "memories_moderate",
                "legacymind_search", "photography_search",
                "maintenance_ops",
                "inner_voice"
            ]},
            "format": {"type": "string", "enum": ["compact", "full"], "default": "full"}
        }
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn photography_memories_schema() -> Arc<Map<String, Value>> {
    // Unified schema that can express create/search/moderate operations
    let schema = json!({
        "type": "object",
        "properties": {
            "mode": {"type": "string", "enum": ["create", "search", "moderate"], "default": "search"},
            // create-like
            "kind": {"type": "string", "enum": ["entity", "relationship", "observation"]},
            "data": {"type": "object"},
            "upsert": {"type": "boolean"},
            "source_thought_id": {"type": "string"},
            "confidence": {"type": ["number", "string"]},
            // search-like
            "query": {"type": "object"},
            "target": {"type": "string", "enum": ["entity", "relationship", "observation", "mixed"]},
            "top_k": {"type": ["integer", "number", "string"], "minimum": 1, "maximum": 50},
            // moderate-like
            "action": {"type": "string", "enum": ["review", "decide", "review_and_decide"]},
            "status": {"type": "string", "enum": ["pending", "approved", "rejected", "auto_approved"]},
            "min_conf": {"type": ["number", "string"], "minimum": 0.0, "maximum": 1.0},
            "limit": {"type": ["integer", "number", "string"], "minimum": 1, "maximum": 200},
            "offset": {"type": ["integer", "number", "string"], "minimum": 0},
            "cursor": {"type": "string"},
            "items": {"type": "array"},
            "dry_run": {"type": "boolean"}
        },
        "required": ["mode"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn maintenance_ops_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "subcommand": {"type": "string", "enum": ["list_removal_candidates", "export_removals", "finalize_removal", "health_check_embeddings", "reembed", "reembed_kg", "ensure_continuity_fields"], "description": "Maintenance operation to perform"},
            "dry_run": {"type": "boolean", "default": false, "description": "Simulate operation without making changes"},
            "limit": {"type": ["integer", "number", "string"], "default": 100, "description": "Maximum number of thoughts to process"},
            "format": {"type": "string", "enum": ["parquet"], "default": "parquet", "description": "Export format (only parquet supported)"},
            "output_dir": {"type": "string", "default": "./archive", "description": "Directory for export files"}
        },
        "required": ["subcommand"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn unified_search_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "query": {"type": "object"},
            "target": {"type": "string", "enum": ["entity", "relationship", "observation", "mixed"], "default": "mixed"},
            "include_thoughts": {"type": "boolean", "default": false},
            "thoughts_content": {"type": "string"},
            "top_k_memories": {"type": ["integer", "number", "string"], "minimum": 1, "maximum": 50, "default": 10},
            "top_k_thoughts": {"type": ["integer", "number", "string"], "minimum": 1, "maximum": 50, "default": 5},
            "sim_thresh": {"type": "number", "minimum": 0.0, "maximum": 1.0}
        }
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

/// Output structs for inner_voice.retrieve
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Snippet {
    pub id: String,
    pub table: String,
    pub source_type: String,
    pub origin: String,
    pub trust_tier: String,
    pub created_at: String,
    pub text: String,
    pub score: f32,
    pub content_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_start: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_end: Option<usize>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Diagnostics {
    pub provider: String,
    pub model: String,
    pub dim: usize,
    pub k_req: usize,
    pub k_ret: usize,
    pub kg_candidates: usize,
    pub thought_candidates: usize,
    pub floor_used: f32,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetrieveOut {
    pub snippets: Vec<Snippet>,
    pub diagnostics: Diagnostics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synth_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synth_model: Option<String>,
}

pub fn inner_voice_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "query": {"type": "string"},
            "top_k": {"type": "integer", "minimum": 1, "maximum": 50, "default": 10},
            "floor": {"type": "number", "minimum": 0.0, "maximum": 1.0},
            "mix": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.6},
            "include_private": {"type": "boolean", "default": false},
            "include_tags": {"type": "array", "items": {"type": "string"}},
            "exclude_tags": {"type": "array", "items": {"type": "string"}},
            "auto_extract_to_kg": {"type": "boolean", "default": false}
        },
        "required": ["query"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

// (photography_search_schema removed in favor of two explicit tools)
