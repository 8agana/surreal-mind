use serde_json::{Map, Value, json};
use std::sync::Arc;

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

pub fn delegate_gemini_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "prompt": {"type": "string"},
            "task_name": {"type": "string"},
            "model": {"type": "string"},
            "cwd": {"type": "string"},
            "timeout_ms": {"type": "number"},
            "fire_and_forget": {"type": "boolean", "default": false}
        },
        "required": ["prompt"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn agent_job_status_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "job_id": {"type": "string"}
        },
        "required": ["job_id"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn list_agent_jobs_schema() -> Arc<Map<String, Value>> {
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

pub fn cancel_agent_job_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "job_id": {"type": "string"}
        },
        "required": ["job_id"]
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

pub fn detailed_help_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "tool": {"type": "string", "enum": [
                "legacymind_think",
                "memories_create",
                "legacymind_search",
                "maintenance_ops",
                "delegate_gemini",
                "curiosity_add",
                "curiosity_get",
                "curiosity_search",
                "detailed_help"
            ]},
            "format": {"type": "string", "enum": ["compact", "full"], "default": "full"}
        }
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn maintenance_ops_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "subcommand": {"type": "string", "enum": ["list_removal_candidates", "export_removals", "finalize_removal", "health_check_embeddings", "health_check_indexes", "reembed", "reembed_kg", "ensure_continuity_fields", "echo_config"], "description": "Maintenance operation to perform"},
            "dry_run": {"type": "boolean", "default": false, "description": "Simulate operation without making changes"},
            "limit": {"type": ["integer", "number", "string"], "default": 100, "description": "Maximum number of thoughts to process"},
            "format": {"type": "string", "enum": ["json", "parquet"], "default": "json", "description": "Export format"},
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
            "order": {"type": "string", "enum": ["created_at_asc", "created_at_desc"]}
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

// (photography_search_schema removed in favor of two explicit tools)

pub fn curiosity_add_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "content": {"type": "string"},
            "tags": {"type": "array", "items": {"type": "string"}},
            "agent": {"type": "string"},
            "topic": {"type": "string"},
            "in_reply_to": {"type": "string"}
        },
        "required": ["content"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn curiosity_get_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 20},
            "since": {"type": "string", "pattern": "^\\d{4}-\\d{2}-\\d{2}$"}
        }
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn curiosity_search_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "query": {"type": "string"},
            "top_k": {"type": "integer", "minimum": 1, "maximum": 50, "default": 10},
            "recency_days": {"type": "integer", "minimum": 1, "maximum": 365}
        },
        "required": ["query"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

// ============================================================================
// OUTPUT SCHEMAS (rmcp 0.11.0+)
// These define the structure of tool responses for schema validation
// ============================================================================

pub fn legacymind_think_output_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "mode_selected": {"type": "string", "description": "The thinking mode that was selected"},
            "reason": {"type": "string", "description": "Why this mode was selected"},
            "delegated_result": {
                "type": "object",
                "properties": {
                    "thought_id": {"type": "string"},
                    "embedding_model": {"type": "string"},
                    "embedding_dim": {"type": "integer"},
                    "memories_injected": {"type": "integer"}
                },
                "description": "Result from the delegated thinking mode"
            },
            "links": {
                "type": "object",
                "properties": {
                    "session_id": {"type": ["string", "null"]},
                    "chain_id": {"type": ["string", "null"]},
                    "previous_thought_id": {"type": ["string", "null"]},
                    "revises_thought": {"type": ["string", "null"]},
                    "branch_from": {"type": ["string", "null"]},
                    "confidence": {"type": ["number", "null"]}
                },
                "description": "Resolved continuity links"
            },
            "telemetry": {
                "type": "object",
                "description": "Trigger matching and heuristic info"
            },
            "verification": {
                "type": "object",
                "properties": {
                    "hypothesis": {"type": "string"},
                    "supporting": {"type": "array"},
                    "contradicting": {"type": "array"},
                    "confidence_score": {"type": "number"},
                    "suggested_revision": {"type": ["string", "null"]}
                },
                "description": "Optional hypothesis verification result"
            }
        },
        "required": ["mode_selected", "reason", "delegated_result", "links", "telemetry"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn delegate_gemini_output_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "oneOf": [
            {
                "type": "object",
                "properties": {
                    "response": {"type": "string", "description": "Model response text"},
                    "session_id": {"type": "string", "description": "Gemini session ID"},
                    "exchange_id": {"type": "string", "description": "Persisted exchange record ID"}
                },
                "required": ["response", "session_id", "exchange_id"],
                "description": "Synchronous response"
            },
            {
                "type": "object",
                "properties": {
                    "status": {"type": "string", "enum": ["queued"], "description": "Job status"},
                    "job_id": {"type": "string", "description": "Job ID for tracking"},
                    "message": {"type": "string", "description": "Status message"}
                },
                "required": ["status", "job_id", "message"],
                "description": "Asynchronous response (fire_and_forget=true)"
            }
        ]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn agent_job_status_output_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "job_id": {"type": "string"},
            "status": {"type": "string", "enum": ["queued", "running", "completed", "failed", "cancelled"]},
            "created_at": {"type": "string"},
            "started_at": {"type": ["string", "null"]},
            "completed_at": {"type": ["string", "null"]},
            "duration_ms": {"type": ["integer", "null"]},
            "error": {"type": ["string", "null"]},
            "session_id": {"type": ["string", "null"]},
            "exchange_id": {"type": ["string", "null"]},
            "metadata": {"type": ["object", "null"]}
        },
        "required": ["job_id", "status", "created_at"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn list_agent_jobs_output_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "jobs": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "job_id": {"type": "string"},
                        "status": {"type": "string"},
                        "tool_name": {"type": "string"},
                        "created_at": {"type": "string"},
                        "completed_at": {"type": ["string", "null"]},
                        "duration_ms": {"type": ["integer", "null"]}
                    }
                }
            },
            "total": {"type": "integer"}
        },
        "required": ["jobs", "total"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn cancel_agent_job_output_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "job_id": {"type": "string"},
            "previous_status": {"type": "string"},
            "new_status": {"type": "string", "enum": ["cancelled"]},
            "message": {"type": "string"}
        },
        "required": ["job_id", "previous_status", "new_status", "message"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn memories_create_output_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "kind": {"type": "string", "enum": ["entity", "relationship", "observation"]},
            "id": {"type": "string", "description": "The created record ID"},
            "created": {"type": "boolean", "description": "True if newly created, false if existing found"}
        },
        "required": ["kind", "id", "created"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn legacymind_search_output_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "memories": {
                "type": "object",
                "properties": {
                    "items": {"type": "array", "description": "Memory items found"}
                },
                "description": "Memory search results"
            },
            "thoughts": {
                "type": "object",
                "properties": {
                    "total": {"type": "integer"},
                    "results": {"type": "array"}
                },
                "description": "Thought search results (when include_thoughts=true)"
            }
        }
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn maintenance_ops_output_schema() -> Arc<Map<String, Value>> {
    // Maintenance ops returns different structures based on subcommand
    // Using a permissive schema that allows any object structure
    let schema = json!({
        "type": "object",
        "additionalProperties": true,
        "description": "Output varies by subcommand. Common fields: expected_dim, thoughts, kg_entities, kg_observations for health_check; candidates, archived for removal ops."
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn detailed_help_output_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "tools": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "one_liner": {"type": "string"},
                        "key_params": {"type": "array", "items": {"type": "string"}}
                    }
                },
                "description": "List of available tools (when no specific tool requested)"
            },
            "name": {"type": "string", "description": "Tool name (when specific tool requested)"},
            "description": {"type": "string"},
            "arguments": {"type": "object"},
            "returns": {"type": "object"},
            "prompts": {
                "type": "array",
                "description": "List of prompts (when prompts=true)"
            }
        }
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}
