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

        let tool = match maybe_tool {
            None => {
                // Canonical tools roster (wrapped in object for rmcp 0.11.0 schema validation)
                let tools = vec![
                    json!({"name": "legacymind_think", "one_liner": "Unified thinking tool with automatic mode routing via triggers/heurs", "key_params": ["content", "hint", "injection_scale", "tags", "significance"]}),
                    json!({"name": "memories_create", "one_liner": "Create entities/relationships/observations in the KG", "key_params": ["kind", "data", "confidence", "source_thought_id"]}),
                    json!({"name": "legacymind_search", "one_liner": "Unified LM search: memories (default) + optional thoughts", "key_params": ["query", "target", "include_thoughts", "top_k_memories", "top_k_thoughts"]}),
                    json!({"name": "maintenance_ops", "one_liner": "Archival, export, re-embed checks and housekeeping", "key_params": ["subcommand", "limit", "dry_run", "output_dir"]}),
                    json!({"name": "delegate_gemini", "one_liner": "Delegate a prompt to the Gemini CLI agent", "key_params": ["prompt", "task_name", "model", "cwd"]}),
                    json!({"name": "curiosity_add", "one_liner": "Add a lightweight note to the curiosity table", "key_params": ["content", "tags", "agent"]}),
                    json!({"name": "curiosity_get", "one_liner": "Get recent curiosity entries", "key_params": ["limit", "since"]}),
                    json!({"name": "curiosity_search", "one_liner": "Search curiosity entries via embeddings", "key_params": ["query", "top_k", "recency_days"]}),
                    json!({"name": "agent_job_status", "one_liner": "Get status of an async agent job", "key_params": ["job_id"]}),
                    json!({"name": "list_agent_jobs", "one_liner": "List async agent jobs", "key_params": ["limit", "status_filter", "tool_name"]}),
                    json!({"name": "cancel_agent_job", "one_liner": "Cancel a running or queued job", "key_params": ["job_id"]}),
                    json!({"name": "detailed_help", "one_liner": "Get help for a specific tool or list all tools", "key_params": ["tool", "format"]}),
                ];
                return Ok(CallToolResult::structured(json!({ "tools": tools })));
            }
            Some(t) => t,
        };

        let help = match tool {
            "legacymind_think" => json!({
                "name": "legacymind_think",
                "description": "Unified thinking tool that routes to appropriate mode. Persists thoughts with optional memory injection. (Framework enhancement currently disabled).",
                "arguments": {
                    "content": "string (required) — the thought text",
                    "hint": "string — optional explicit mode ('debug', 'build', 'plan', 'stuck', 'question', 'conclude')",
                    "injection_scale": "integer|string (0-3) — memory injection level (overrides mode default)",
                    "tags": "string[] — optional tags",
                    "significance": "number|string (0.0-1.0) — importance (overrides mode default)",
                    "verbose_analysis": "boolean — (unused) previously for verbose framework output",
                    "session_id": "string — optional session identifier",
                    "chain_id": "string — optional chain identifier",
                    "previous_thought_id": "string — optional reference to previous thought",
                    "revises_thought": "string — optional reference to thought being revised",
                    "branch_from": "string — optional reference to thought being branched from",
                    "confidence": "number (0.0-1.0) — optional confidence level",
                    "hypothesis": "string — optional hypothesis to verify against KG evidence",
                    "needs_verification": "boolean — set true to run hypothesis verification (only when hypothesis provided)",
                    "verify_top_k": "integer (1-500) — candidate pool size for KG search (default 100)",
                    "min_similarity": "number (0.0-1.0) — minimum similarity threshold (default 0.70)",
                    "evidence_limit": "integer (1-25) — max evidence items per bucket (default 10)",
                    "contradiction_patterns": "string[] — optional custom patterns for contradiction detection"
                },
                "returns": {
                    "thought_id": "string — the ID of the created thought",
                    "memories_injected": "integer — count of memories injected",
                    "embedding_dim": "integer — dimension of the generated embedding",
                    "embedding_model": "string — model used for embedding",
                    "continuity": {
                        "session_id": "string? — resolved session identifier",
                        "chain_id": "string? — resolved chain identifier",
                        "previous_thought_id": "string? — resolved previous thought reference",
                        "revises_thought": "string? — resolved thought being revised",
                        "branch_from": "string? — resolved branch reference",
                        "confidence": "number? — clamped confidence value",
                        "links_resolved": "object? — details on how links were resolved"
                    },
                    "verification": "object? — hypothesis verification result"
                },
                 "routing": {
                     "triggers": {
                         "debug": "debug time",
                         "build": "building time",
                         "plan": "plan/planning time",
                         "stuck": "i'm stuck / stuck",
                         "question": "question time",
                         "conclude": "wrap up / conclude"
                     },
                     "heuristics": {
                         "debug": ["error", "bug", "stack trace", "failed", "exception", "panic"],
                         "build": ["implement", "create", "add function", "build", "scaffold", "wire"],
                         "plan": ["architecture", "design", "approach", "how should", "strategy", "trade-off"],
                         "stuck": ["stuck", "unsure", "confused", "not sure", "blocked"]
                     }
                 }
            }),
            "legacymind_search" => json!({
                "name": "legacymind_search",
                "description": "Unified search in LegacyMind: searches memories by default and, when include_thoughts=true, also searches thoughts. Supports continuity field filters for thoughts.",
                "arguments": {
                    "query": "object — {name?, text?} query parameters",
                    "target": "'entity'|'relationship'|'observation'|'mixed' (default 'mixed')",
                    "include_thoughts": "boolean (default false) — also search thoughts",
                    "thoughts_content": "string — optional explicit query text for thoughts",
                    "top_k_memories": "integer (1-50; default 10)",
                    "top_k_thoughts": "integer (1-50; default 5)",
                    "sim_thresh": "number (0.0-1.0) — similarity floor for thoughts",
                    "session_id": "string? — filter thoughts by session_id",
                    "chain_id": "string? — filter thoughts by chain_id",
                    "previous_thought_id": "string? — filter thoughts by previous_thought_id (record or string)",
                    "revises_thought": "string? — filter thoughts by revises_thought (record or string)",
                    "branch_from": "string? — filter thoughts by branch_from (record or string)",
                    "origin": "string? — filter thoughts by origin",
                    "confidence_gte": "number? (0.0-1.0) — filter thoughts with confidence >= value",
                    "confidence_lte": "number? (0.0-1.0) — filter thoughts with confidence <= value",
                    "date_from": "string? (YYYY-MM-DD) — filter thoughts created_at >= date",
                    "date_to": "string? (YYYY-MM-DD) — filter thoughts created_at <= date",
                    "order": "string? ('created_at_asc'|'created_at_desc') — order thoughts by created_at"
                },
                "returns": {"memories": {"items": "array"}, "thoughts": {"total": "number", "results": "array"}},
                "examples": [
                    {"description": "Search thoughts in a specific session, ordered by creation time", "call": {"include_thoughts": true, "session_id": "session_123"}},
                    {"description": "Search thoughts in a chain with similarity ordering", "call": {"include_thoughts": true, "chain_id": "chain_456", "thoughts_content": "debug issue"}},
                    {"description": "Find thoughts that revise a specific thought", "call": {"include_thoughts": true, "revises_thought": "thoughts:789"}},
                    {"description": "Search thoughts with confidence >= 0.8 in a date range", "call": {"include_thoughts": true, "confidence_gte": 0.8, "date_from": "2024-01-01", "date_to": "2024-12-31"}}
                ]
            }),
            "memories_create" => json!({
                "name": "memories_create",
                "description": "Create personal memory entities or relationships; returns created id.",
                "arguments": {
                    "kind": "string — 'entity'|'relationship'|'observation'",
                    "data": "object — entity: {name, entity_type?, properties?} | relationship: {source, target, rel_type, properties?} | observation: {source, observation_type, properties?}",
                    "confidence": "number — optional confidence",
                    "upsert": "boolean (default true) — whether to find existing matching record or always create new"
                },
                "returns": {"created": true, "id": "string", "kind": "string"}
            }),
            "maintenance_ops" => json!({
                "name": "maintenance_ops",
                "description": "Maintenance operations for archival, cleanup, and health checks.",
                "arguments": {
                    "subcommand": "string (required) — 'list_removal_candidates'|'export_removals'|'finalize_removal'|'health_check_embeddings'|'health_check_indexes'|'reembed'|'reembed_kg'|'ensure_continuity_fields'|'echo_config'",
                    "dry_run": "boolean (default: false) — simulate operation without changes",
                    "limit": "integer|string (default: 100) — max items to process",
                    "format": "string (default: 'json') — export format",
                    "output_dir": "string (default: './archive') — export directory"
                },
                "returns": {"depends on subcommand": "object with counts, paths, or messages"}
            }),
            "delegate_gemini" => json!({
                "name": "delegate_gemini",
                "description": "Delegate a prompt to the Gemini CLI agent as an async background job.",
                "arguments": {
                    "prompt": "string (required) — the prompt text",
                    "task_name": "string (default 'delegate_gemini') — groups related operations",
                    "model": "string (default 'auto') — override model selection",
                    "cwd": "string — working directory for the agent",
                    "timeout_ms": "integer — override global timeout",
                    "tool_timeout_ms": "integer — per-tool execution timeout",
                    "expose_stream": "boolean — whether to stream output (if supported)"
                },
                "returns": {"status": "queued", "job_id": "string", "message": "string"}
            }),
            "curiosity_add" => json!({
                "name": "curiosity_add",
                "description": "Add a lightweight note to the curiosity table",
                "arguments": {
                    "content": "string (required) — the content of the note",
                    "tags": "string[] — optional tags",
                    "agent": "string — optional agent identifier",
                    "topic": "string — optional topic classification",
                    "in_reply_to": "string — optional reference ID"
                },
                "returns": {"id": "string"}
            }),
            "curiosity_get" => json!({
                "name": "curiosity_get",
                "description": "Get recent curiosity entries",
                "arguments": {
                    "limit": "integer (default 20) — max entries to return",
                    "since": "string (YYYY-MM-DD) — optional date filter"
                },
                "returns": {"entries": "array"}
            }),
            "curiosity_search" => json!({
                "name": "curiosity_search",
                "description": "Search curiosity entries via embeddings",
                "arguments": {
                    "query": "string (required) — search query",
                    "top_k": "integer (default 10) — max results",
                    "recency_days": "integer — optionally limit to recent days"
                },
                "returns": {"results": "array", "snippets": "array"}
            }),
            "agent_job_status" => json!({
                "name": "agent_job_status",
                "description": "Get status of an async agent job",
                "arguments": {
                    "job_id": "string (required)"
                },
                "returns": {
                    "job_id": "string",
                    "status": "queued|running|completed|failed|cancelled",
                    "created_at": "string",
                    "started_at": "string?",
                    "completed_at": "string?",
                    "duration_ms": "integer?",
                    "error": "string?",
                    "session_id": "string?",
                    "exchange_id": "string?",
                    "metadata": "object?"
                }
            }),
            "list_agent_jobs" => json!({
                "name": "list_agent_jobs",
                "description": "List async agent jobs with optional filtering",
                "arguments": {
                    "limit": "integer (default 20)",
                    "status_filter": "string — optional status to filter by",
                    "tool_name": "string — optional tool name to filter by"
                },
                "returns": {
                    "jobs": "array of job summaries",
                    "total": "integer"
                }
            }),
            "cancel_agent_job" => json!({
                "name": "cancel_agent_job",
                "description": "Cancel a running or queued async agent job",
                "arguments": {
                    "job_id": "string (required)"
                },
                "returns": {
                    "job_id": "string",
                    "previous_status": "string",
                    "new_status": "string",
                    "message": "string"
                }
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
