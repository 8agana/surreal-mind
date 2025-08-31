//! detailed_help tool handler to provide structured help for tools

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

impl SurrealMindServer {
    /// Handle the detailed_help tool call
    pub async fn handle_detailed_help(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;

        let tool = args.get("tool").and_then(|v| v.as_str()).ok_or_else(|| {
            SurrealMindError::Validation {
                message: "'tool' parameter is required".into(),
            }
        })?;
        let format = args
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("full");

        let help = match tool {
            "convo_think" => json!({
                "name": "convo_think",
                "description": "Store conversational thoughts with optional memory injection and submodes.",
                "arguments": {
                    "content": "string (required) — the thought text",
                    "injection_scale": "integer|string (0-5 or presets) — memory injection level",
                    "submode": "string — e.g., 'sarcastic' (default)",
                    "tags": "string[] — optional tags",
                    "significance": "number|string (0.0-1.0 or presets) — importance"
                },
                "returns": {"thought_id": "string", "memories_injected": "number", "submode_used": "string"},
                "examples": [{
                    "request": {"name": "convo_think", "arguments": {"content": "Note this idea.", "injection_scale": 2}},
                    "response": {"thought_id": "...", "memories_injected": 0, "submode_used": "sarcastic"}
                }]
            }),
            "tech_think" => json!({
                "name": "tech_think",
                "description": "Technical reasoning with memory injection; defaults submode to 'plan'.",
                "arguments": {
                    "content": "string (required)",
                    "injection_scale": "integer|string (0-5 or presets)",
                    "submode": "string — 'plan'|'build'|'debug' (default: 'plan')",
                    "tags": "string[]",
                    "significance": "number|string (0.0-1.0)"
                },
                "returns": {"thought_id": "string", "memories_injected": "number", "submode_used": "string"}
            }),
            "inner_voice" => json!({
                "name": "inner_voice",
                "description": "Private inner thoughts; stored with is_inner_voice=true and visibility controls.",
                "arguments": {
                    "content": "string (required)",
                    "injection_scale": "integer|string (0-5 or presets)",
                    "significance": "number|string (0.0-1.0)",
                    "inner_visibility": "string — 'private'|'context_only' (default: 'context_only')"
                },
                "returns": {"thought_id": "string", "inner_voice": true, "visibility": "string"}
            }),
            "search_thoughts" => json!({
                "name": "search_thoughts",
                "description": "Semantic search over stored thoughts; computes similarity client-side.",
                "arguments": {
                    "content": "string (required) — query text",
                    "top_k": "integer — max results (1-50; default from env SURR_TOP_K)",
                    "offset": "integer — pagination offset",
                    "sim_thresh": "number — minimum similarity (0.0-1.0; default SURR_SIM_THRESH)",
                    "submode": "string — filter by submode",
                    "min_significance": "number — filter by significance",
                    "expand_graph": "boolean — (reserved)",
                    "graph_depth": "integer — (reserved)",
                    "graph_boost": "number — (reserved)",
                    "min_edge_strength": "number — (reserved)",
                    "sort_by": "string — 'score'|'similarity'|'recency'|'significance'"
                },
                "returns": {"total": "number", "offset": "number", "top_k": "number", "results": "array"}
            }),
            "knowledgegraph_create" => json!({
                "name": "knowledgegraph_create",
                "description": "Create KG entities or relationships; returns created id.",
                "arguments": {
                    "kind": "string — 'entity'|'relationship'",
                    "data": "object — entity: {name, entity_type?, properties?} | relationship: {source, target, rel_type, properties?}",
                    "confidence": "number — optional confidence"
                },
                "returns": {"created": true, "id": "string", "kind": "string"}
            }),
            "knowledgegraph_search" => json!({
                "name": "knowledgegraph_search",
                "description": "Search KG entities/relationships by name substring; returns items.",
                "arguments": {"target": "'entity'|'relationship'|'mixed'", "query": "object — {name?}", "top_k": "integer"},
                "returns": {"items": "array"}
            }),
            "knowledgegraph_moderate" => json!({
                "name": "knowledgegraph_moderate",
                "description": "Unified moderation: review candidates and/or apply decisions in one call.",
                "arguments": {
                    "action": "'review'|'decide'|'review_and_decide' (default: 'review')",
                    "target": "'entity'|'relationship'|'mixed' (default: 'mixed')",
                    "status": "'pending'|'approved'|'rejected'|'auto_approved' (default: 'pending')",
                    "min_conf": "number — minimum confidence filter",
                    "limit": "integer — page size",
                    "offset": "integer — page offset",
                    "cursor": "string — (reserved)",
                    "items": "array — decisions: [{id, kind, decision, feedback?, canonical_id?}]",
                    "dry_run": "boolean — simulate decisions without writes"
                },
                "returns": {"review": {"items": "array"}, "results": "array"}
            }),
            _ => {
                return Err(SurrealMindError::Validation {
                    message: format!("Unknown tool: {}", tool),
                });
            }
        };

        let output = if format == "compact" {
            // Provide a concise one-paragraph summary
            json!({
                "tool": tool,
                "summary": help.get("description").cloned().unwrap_or(json!("")),
                "arguments": help.get("arguments").cloned().unwrap_or(json!({}))
            })
        } else {
            help
        };

        Ok(CallToolResult::structured(output))
    }
}
