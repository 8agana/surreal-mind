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
            // New think tools
            "think_convo" => json!({
                "name": "think_convo",
                "description": "Store conversational thoughts with optional memory injection.",
                "arguments": {
                    "content": "string (required) — the thought text",
                    "injection_scale": "integer|string (0-5 or presets) — memory injection level",
                    "tags": "string[] — optional tags",
                    "significance": "number|string (0.0-1.0 or presets) — importance"
                },
                "returns": {"thought_id": "string", "memories_injected": "number", "embedding_model": "string", "embedding_dim": "number"}
            }),
            "think_plan" => json!({
                "name": "think_plan",
                "description": "Architecture and strategy thinking (systems_thinking). High context injection.",
                "arguments": {
                    "content": "string (required)",
                    "injection_scale": "integer|string (default: 3)",
                    "significance": "number|string (default: 0.7)",
                    "tags": "string[]"
                },
                "returns": {"thought_id": "string", "memories_injected": "number", "embedding_model": "string", "embedding_dim": "number"}
            }),
            "think_debug" => json!({
                "name": "think_debug",
                "description": "Problem solving (root_cause_analysis). Maximum context injection.",
                "arguments": {
                    "content": "string (required)",
                    "injection_scale": "integer|string (default: 4)",
                    "significance": "number|string (default: 0.8)",
                    "tags": "string[]"
                },
                "returns": {"thought_id": "string", "memories_injected": "number", "embedding_model": "string", "embedding_dim": "number"}
            }),
            "think_build" => json!({
                "name": "think_build",
                "description": "Implementation thinking (incremental). Focused context injection.",
                "arguments": {
                    "content": "string (required)",
                    "injection_scale": "integer|string (default: 2)",
                    "significance": "number|string (default: 0.6)",
                    "tags": "string[]"
                },
                "returns": {"thought_id": "string", "memories_injected": "number", "embedding_model": "string", "embedding_dim": "number"}
            }),
            "think_stuck" => json!({
                "name": "think_stuck",
                "description": "Breaking through blocks (lateral_thinking). Varied context injection.",
                "arguments": {
                    "content": "string (required)",
                    "injection_scale": "integer|string (default: 3)",
                    "significance": "number|string (default: 0.9)",
                    "tags": "string[]"
                },
                "returns": {"thought_id": "string", "memories_injected": "number", "embedding_model": "string", "embedding_dim": "number"}
            }),
            // Legacy aliases for help
            "convo_think" => json!({"alias_of": "think_convo"}),
            "tech_think" => json!({"alias_of": "think_plan"}),
            "inner_voice" => json!({
                "name": "inner_voice",
                "description": "RAG query tool: retrieves relevant thoughts, synthesizes answer from sources, optionally stages KG candidates from retrieved thoughts; saves summary thought by default.",
                "arguments": {
                    "content": "string (required) — query text",
                    "top_k": "integer|string (1-50, default: 5) — max thoughts to retrieve",
                    "sim_thresh": "number (0.0-1.0, default: 0.5) — similarity threshold",
                    "stage_kg": "boolean (default: false) — stage candidates from retrieved thoughts",
                    "confidence_min": "number (0.0-1.0, default: 0.6) — staging threshold",
                    "max_nodes": "integer|string (default: 30) — max entities to stage",
                    "max_edges": "integer|string (default: 60) — max relationships to stage",
                    "save": "boolean (default: true) — persist synthesized summary thought",
                    "auto_mark_removal": "boolean (default: false) — set sources to status='removal' after staging"
                },
                "returns": {"synthesized_answer": "string", "saved_thought_id": "string?", "sources": "array", "staged": "object", "marked_for_removal": "number"}
            }),
            "think_search" => json!({
                "name": "think_search",
                "description": "Semantic search over stored thoughts; computes similarity client-side.",
                "arguments": {
                    "content": "string (required) — query text",
                    "top_k": "integer — max results (1-50; default from env SURR_TOP_K)",
                    "offset": "integer — pagination offset",
                    "sim_thresh": "number — minimum similarity (0.0-1.0; default SURR_SIM_THRESH)",
                    
                    "min_significance": "number — filter by significance",
                    "expand_graph": "boolean — (reserved)",
                    "graph_depth": "integer — (reserved)",
                    "graph_boost": "number — (reserved)",
                    "min_edge_strength": "number — (reserved)",
                    "sort_by": "string — 'score'|'similarity'|'recency'|'significance'"
                },
                "returns": {"total": "number", "offset": "number", "top_k": "number", "results": "array"}
            }),
            // Legacy alias
            "search_thoughts" => json!({"alias_of": "think_search"}),
            "memories_create" => json!({
                "name": "memories_create",
                "description": "Create personal memory entities or relationships; returns created id.",
                "arguments": {
                    "kind": "string — 'entity'|'relationship'",
                    "data": "object — entity: {name, entity_type?, properties?} | relationship: {source, target, rel_type, properties?}",
                    "confidence": "number — optional confidence"
                },
                "returns": {"created": true, "id": "string", "kind": "string"}
            }),
            "memories_search" => json!({
                "name": "memories_search",
                "description": "Search personal memory entities/relationships; returns items.",
                "arguments": {"target": "'entity'|'relationship'|'mixed'", "query": "object — {name?}", "top_k": "integer"},
                "returns": {"items": "array"}
            }),
            "memories_moderate" => json!({
                "name": "memories_moderate",
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
            // Legacy aliases for KG help
            "knowledgegraph_create" => json!({"alias_of": "memories_create"}),
            "knowledgegraph_search" => json!({"alias_of": "memories_search"}),
            "knowledgegraph_moderate" => json!({"alias_of": "memories_moderate"}),
            "maintenance_ops" => json!({
                "name": "maintenance_ops",
                "description": "Maintenance operations for archival and cleanup of thoughts.",
                "arguments": {
                    "subcommand": "string (required) — 'list_removal_candidates'|'export_removals'|'finalize_removal'",
                    "dry_run": "boolean (default: false) — simulate operation without changes",
                    "limit": "integer|string (default: 100) — max items to process",
                    "format": "string (default: 'parquet') — export format",
                    "output_dir": "string (default: './archive') — export directory"
                },
                "returns": {"depends on subcommand": "object with counts, paths, or messages"}
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
