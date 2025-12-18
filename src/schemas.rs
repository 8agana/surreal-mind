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
                "legacymind_think",
                "memories_create", "memories_moderate",
                "legacymind_search",
                "maintenance_ops", "inner_voice",
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
            "auto_extract_to_kg": {"type": "boolean", "default": false},
            "previous_thought_id": {"type": "string"},
            "include_feedback": {"type": "boolean", "default": true},
            "feedback_max_lines": {"type": "integer", "default": 3, "minimum": 1, "maximum": 10},
            "recency_days": {"type": "integer", "minimum": 1, "maximum": 365},
            "prefer_recent": {"type": "boolean"}
        },
        "required": ["query"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

// (photography_search_schema removed in favor of two explicit tools)

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

pub fn memories_moderate_output_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "review": {
                "type": "object",
                "properties": {
                    "items": {"type": "array", "description": "Candidates for review"}
                },
                "description": "Review results (when action includes review)"
            },
            "results": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "kind": {"type": "string"},
                        "decision": {"type": "string"},
                        "success": {"type": "boolean"},
                        "error": {"type": "string"}
                    }
                },
                "description": "Decision results (when action includes decide)"
            }
        }
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

pub fn inner_voice_output_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "answer": {"type": "string", "description": "Synthesized answer from memories/thoughts"},
            "synth_thought_id": {"type": "string", "description": "ID of the synthesis thought created"},
            "feedback": {"type": "string", "description": "Optional feedback text"},
            "feedback_thought_id": {"type": ["string", "null"], "description": "ID of feedback thought if created"},
            "sources_compact": {"type": "string", "description": "Compact list of source IDs"},
            "synth_provider": {"type": "string", "description": "Provider used for synthesis"},
            "synth_model": {"type": "string", "description": "Model used for synthesis"},
            "embedding_dim": {"type": "integer", "description": "Embedding dimension used"},
            "extracted": {
                "type": "object",
                "properties": {
                    "entities": {"type": "integer"},
                    "relationships": {"type": "integer"}
                },
                "description": "Count of entities/relationships extracted to KG (when auto_extract_to_kg=true)"
            }
        },
        "required": ["answer", "synth_thought_id", "sources_compact", "synth_provider", "synth_model", "embedding_dim", "extracted"]
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
