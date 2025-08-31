use serde_json::{Map, Value, json};
use std::sync::Arc;

pub fn convo_think_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "content": {"type": "string"},
            "injection_scale": {"type": ["integer", "string"]},
            "submode": {"type": "string"},
            "tags": {"type": "array", "items": {"type": "string"}},
            "significance": {"type": ["number", "string"]},
            "verbose_analysis": {"type": "boolean"}
        },
        "required": ["content"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn tech_think_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "content": {"type": "string"},
            "injection_scale": {"type": ["integer", "string"]},
            "submode": {"type": "string", "enum": ["plan", "build", "debug"], "default": "plan"},
            "tags": {"type": "array", "items": {"type": "string"}},
            "significance": {"type": ["number", "string"]},
            "verbose_analysis": {"type": "boolean"}
        },
        "required": ["content"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}

pub fn inner_voice_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "content": {"type": "string"},
            "injection_scale": {"type": ["integer", "string"]},
            "tags": {"type": "array", "items": {"type": "string"}},
            "significance": {"type": ["number", "string"]},
            "verbose_analysis": {"type": "boolean"},
            "inner_visibility": {"type": "string", "enum": ["private", "context_only"], "default": "context_only"},
            "extract_to_kg": {"type": "boolean", "default": false},
            "session_hours": {"type": ["number", "string"], "default": 6},
            "dry_run": {"type": "boolean", "default": false},
            "confidence_min": {"type": ["number", "string"], "minimum": 0.0, "maximum": 1.0, "default": 0.6},
            "max_nodes": {"type": ["integer", "number", "string"], "default": 30},
            "max_edges": {"type": ["integer", "number", "string"], "default": 60}
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
            "submode": {"type": "string"},
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
                        "decision": {"type": "string", "enum": ["approve", "reject"]},
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
                "convo_think", "tech_think", "inner_voice", "search_thoughts", "knowledgegraph_create", "knowledgegraph_search", "knowledgegraph_moderate"
            ]},
            "format": {"type": "string", "enum": ["compact", "full"], "default": "full"}
        }
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}
