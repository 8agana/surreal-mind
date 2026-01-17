use serde_json::{Map, Value, json};
use std::sync::Arc;

pub fn think_schema() -> Arc<Map<String, Value>> {
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

pub fn call_gem_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "prompt": {"type": "string"},
            "task_name": {"type": "string"},
            "model": {"type": "string"},
            "cwd": {"type": "string"},
            "resume_session_id": {"type": "string"},
            "continue_latest": {"type": "boolean", "default": false},
            "timeout_ms": {"type": "number"},
            "tool_timeout_ms": {"type": "number"},
            "expose_stream": {"type": "boolean"},
            "fire_and_forget": {"type": "boolean", "default": false}
        },
        "required": ["prompt"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn call_codex_schema() -> Arc<Map<String, Value>> {
    // Read available models from env var (comma-separated) or use fallback
    let models: Vec<Value> = std::env::var("CODEX_MODELS")
        .map(|s| {
            s.split(',')
                .map(|m| Value::String(m.trim().to_string()))
                .collect()
        })
        .unwrap_or_else(|_| {
            vec![
                Value::String("gpt-5.2-codex".to_string()),
                Value::String("gpt-5.1-codex-max".to_string()),
                Value::String("gpt-5.1-codex-mini".to_string()),
                Value::String("gpt-5.2".to_string()),
            ]
        });

    let default_model =
        std::env::var("CODEX_MODEL").unwrap_or_else(|_| "gpt-5.2-codex".to_string());

    let schema = json!({
        "type": "object",
        "properties": {
            "prompt": {"type": "string"},
            "task_name": {"type": "string", "default": "call_codex"},
            "model": {
                "type": "string",
                "enum": models,
                "default": default_model
            },
            "cwd": {"type": "string"},
            "resume_session_id": {"type": "string"},
            "continue_latest": {"type": "boolean", "default": false},
            "timeout_ms": {"type": "number", "default": 60000},
            "tool_timeout_ms": {"type": "number", "default": 300000},
            "expose_stream": {"type": "boolean", "default": false},
            "fire_and_forget": {"type": "boolean", "default": false}
        },
        "required": ["prompt", "cwd"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn call_status_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "job_id": {"type": "string"}
        },
        "required": ["job_id"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn call_jobs_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 20},
            "status_filter": {"type": "string", "enum": ["queued", "running", "completed", "failed", "cancelled"]},
            "tool_name": {"type": "string"}
        }
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn call_cancel_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "job_id": {"type": "string"}
        },
        "required": ["job_id"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn remember_schema() -> Arc<Map<String, Value>> {
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

pub fn howto_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "tool": {"type": "string", "enum": [
                "think",
                "remember",
                "search",
                "maintain",
                "call_gem",
                "call_codex",
                "call_status",
                "call_jobs",
                "call_cancel",
                "wander",
                "howto"
            ]},
            "format": {"type": "string", "enum": ["compact", "full"], "default": "full"}
        }
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn maintain_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "subcommand": {"type": "string", "enum": ["list_removal_candidates", "export_removals", "finalize_removal", "health_check_embeddings", "health_check_indexes", "reembed", "reembed_kg", "embed_pending", "ensure_continuity_fields", "echo_config", "corrections", "rethink", "populate", "embed", "wander", "health", "report", "tasks"], "description": "Maintenance operation to perform"},
            "dry_run": {"type": "boolean", "default": false, "description": "Simulate operation without making changes"},
            "limit": {"type": ["integer", "number", "string"], "default": 100, "description": "Maximum number of thoughts to process"},
            "format": {"type": "string", "enum": ["json", "parquet"], "default": "json", "description": "Export format"},
            "output_dir": {"type": "string", "default": "./archive", "description": "Directory for export files"},
            "tasks": {"type": "string", "description": "Comma-separated tasks for subcommand 'tasks'"},
            "target_id": {"type": "string", "description": "Optional target filter (corrections subcommand)"},
            "rethink_types": {"type": "string", "description": "Comma-separated mark types (rethink subcommand)"}
        },
        "required": ["subcommand"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn search_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "query": {"type": "object"},
            "target": {"type": "string", "enum": ["entity", "relationship", "observation", "mixed"], "default": "mixed"},
            "include_thoughts": {"type": "boolean", "default": false},
            "thoughts_content": {"type": "string"},
            "top_k_memories": {"type": ["integer", "number", "string"], "minimum": 1, "maximum": 50, "default": 10},
            "top_k_thoughts": {"type": ["integer", "number", "string"], "minimum": 1, "maximum": 50, "default": 5},
            "sim_thresh": {"type": "number", "minimum": 0.0, "maximum": 1.0},
            "session_id": {"type": "string"},
            "chain_id": {"type": "string"},
            "previous_thought_id": {"type": "string"},
            "revises_thought": {"type": "string"},
            "branch_from": {"type": "string"},
            "origin": {"type": "string"},
            "confidence_gte": {"type": "number", "minimum": 0.0, "maximum": 1.0},
            "confidence_lte": {"type": "number", "minimum": 0.0, "maximum": 1.0},
            "date_from": {"type": "string", "pattern": "^\\d{4}-\\d{2}-\\d{2}$"},
            "date_to": {"type": "string", "pattern": "^\\d{4}-\\d{2}-\\d{2}$"},
            "order": {"type": "string", "enum": ["created_at_asc", "created_at_desc"]},
            "forensic": {"type": "boolean", "description": "Return provenance: correction chain, derivatives, sources"}
        }
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

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

// (curiosity_add_schema, curiosity_get_schema, curiosity_search_schema removed - tools deleted)

// Note: Output schemas (legacymind_think_output_schema, etc.) were defined for rmcp 0.11.0+
// but never used. Removed in 0.7.5 cleanup to reduce dead code.

pub fn wander_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "current_thought_id": {"type": "string", "description": "Optional starting point thought/entity ID"},
            "mode": {"type": "string", "enum": ["random", "semantic", "meta", "marks"], "description": "Traversal mode"},
            "visited_ids": {"type": "array", "items": {"type": "string"}, "description": "IDs to avoid preventing loops"},
            "recency_bias": {"type": "boolean", "default": false, "description": "Whether to prioritize recent memories"},
            "for": {"type": "string", "enum": ["cc", "sam", "gemini", "dt", "gem"], "description": "Filter marks assigned to a specific federation member (marks mode only)"}
        },
        "required": ["mode"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn corrections_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "target_id": {"type": "string", "description": "Optional filter: correction events for this target id"},
            "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 10, "description": "Max events to return"}
        }
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn rethink_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "target_id": {"type": "string", "description": "ID of the record to mark (thoughts:xxx, entity:xxx, observation:xxx)"},
            "mode": {"type": "string", "enum": ["mark", "correct"], "description": "Operation mode"},
            "mark_type": {"type": "string", "enum": ["correction", "research", "enrich", "expand"], "description": "Type of mark (mark mode)"},
            "marked_for": {"type": "string", "enum": ["cc", "sam", "gemini", "dt", "gem"], "description": "Target federation member (mark mode)"},
            "note": {"type": "string", "description": "Contextual explanation for the mark (mark mode)"},
            "reasoning": {"type": "string", "description": "Why the record is being corrected (correct mode)"},
            "sources": {"type": "array", "items": {"type": "string"}, "description": "Verification sources (correct mode)"},
            "cascade": {"type": "boolean", "description": "If true, flag derivatives for review", "default": false}
        },
        "required": ["target_id", "mode"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}
