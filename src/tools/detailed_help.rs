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
        let args = request.arguments.unwrap_or_default();

        // Overview mode: when no 'tool' param provided, return a compact roster
        let maybe_tool = args.get("tool").and_then(|v| v.as_str());
        let format = args
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("full");
        // Prompt registry view
        if args
            .get("prompts")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            let reg = crate::prompts::PromptRegistry::new();
            let list = reg
                .list()
                .into_iter()
                .map(|p| {
                    json!({
                        "id": p.id,
                        "one_liner": p.one_liner,
                        "version": p.version,
                        "checksum": p.lineage.checksum,
                        "inputs": p.inputs,
                    })
                })
                .collect::<Vec<_>>();
            return Ok(CallToolResult::structured(json!({ "prompts": list })));
        }

        if maybe_tool.is_none() {
            // Canonical tools roster
            let overview = json!([
                {"name": "think_convo", "one_liner": "Store a conversational thought with optional memory injection", "key_params": ["content", "injection_scale", "significance", "tags"]},
                {"name": "think_plan", "one_liner": "Architecture/strategy thinking (high context)", "key_params": ["content", "injection_scale", "significance", "tags"]},
                {"name": "think_debug", "one_liner": "Root cause analysis (maximum context)", "key_params": ["content", "injection_scale", "significance", "tags"]},
                {"name": "think_build", "one_liner": "Implementation-focused thinking (focused context)", "key_params": ["content", "injection_scale", "significance", "tags"]},
                {"name": "think_stuck", "one_liner": "Lateral thinking to unblock progress", "key_params": ["content", "injection_scale", "significance", "tags"]},
                {"name": "think_search", "one_liner": "Semantic search over thoughts with optional graph expansion", "key_params": ["content", "top_k", "sim_thresh", "offset"]},
                {"name": "memories_create", "one_liner": "Create entities/relationships/observations in the KG", "key_params": ["kind", "data", "confidence", "source_thought_id"]},
                {"name": "memories_search", "one_liner": "Search the Knowledge Graph", "key_params": ["target", "query", "top_k"]},
                {"name": "memories_moderate", "one_liner": "Review/decide on KG candidates", "key_params": ["action", "target", "status", "items", "dry_run"]},
                {"name": "maintenance_ops", "one_liner": "Archival, export, re-embed checks and housekeeping", "key_params": ["subcommand", "limit", "dry_run", "output_dir"]}
            ]);
            return Ok(CallToolResult::structured(overview));
        }

        let tool = maybe_tool.unwrap();

        let help = match tool {
            "think_convo" | "think_plan" | "think_debug" | "think_build" | "think_stuck" => json!({
                "name": "think_*",
                "description": "Stores a thought with memory injection. Different tool names provide different default injection levels.",
                "arguments": {
                    "content": "string (required) — the thought text",
                    "injection_scale": "integer|string (0-5 or presets) — memory injection level (overrides tool default)",
                    "tags": "string[] — optional tags",
                    "significance": "number|string (0.0-1.0 or presets) — importance (overrides tool default)"
                },
                "returns": {"thought_id": "string", "memories_injected": "number", "embedding_model": "string", "embedding_dim": "number"}
            }),
            "think_search" => json!({
                "name": "think_search",
                "description": "Semantic search over stored thoughts.",
                "arguments": {
                    "content": "string (required) — query text",
                    "top_k": "integer — max results (1-50; default from env SURR_TOP_K)",
                    "offset": "integer — pagination offset",
                    "sim_thresh": "number — minimum similarity (0.0-1.0; default SURR_SIM_THRESH)",
                    "min_significance": "number — filter by significance",
                    "sort_by": "string — 'score'|'similarity'|'recency'|'significance'"
                },
                "returns": {"total": "number", "offset": "number", "top_k": "number", "results": "array"}
            }),
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
                    "items": "array — decisions: [{id, kind, decision, feedback?, canonical_id?}]",
                    "dry_run": "boolean — simulate decisions without writes"
                },
                "returns": {"review": {"items": "array"}, "results": "array"}
            }),
            "maintenance_ops" => json!({
                "name": "maintenance_ops",
                "description": "Maintenance operations for archival, health checks, and cleanup.",
                "arguments": {
                    "subcommand": "string (required) — 'list_removal_candidates', 'export_removals', 'finalize_removal', 'health_check_embeddings', 'health_check_indexes', 'reembed', 'reembed_kg'",
                    "dry_run": "boolean (default: false) — simulate operation without changes",
                    "limit": "integer|string (default: 100) — max items to process",
                    "format": "string (default: 'parquet') — export format",
                    "output_dir": "string (default: './archive') — export directory"
                },
                "returns": {"depends on subcommand": "object with counts, paths, or messages"}
            }),
            _ => {
                // Also allow prompt lookup by id via prompt_id param
                if let Some(prompt_id) = args.get("prompt_id").and_then(|v| v.as_str()) {
                    let reg = crate::prompts::PromptRegistry::new();
                    if let Some(p) = reg.get(prompt_id) {
                        let out = if format == "compact" {
                            json!({
                                "id": p.id,
                                "one_liner": p.one_liner,
                                "version": p.version,
                                "checksum": p.lineage.checksum,
                                "inputs": p.inputs,
                            })
                        } else {
                            json!({
                                "id": p.id,
                                "one_liner": p.one_liner,
                                "purpose": p.purpose,
                                "inputs": p.inputs,
                                "constraints": p.constraints,
                                "version": p.version,
                                "lineage": p.lineage,
                                "template": p.template,
                            })
                        };
                        return Ok(CallToolResult::structured(out));
                    }
                }
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
