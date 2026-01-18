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
                    json!({"name": "think", "one_liner": "Unified thinking tool with automatic mode routing via triggers/heurs", "key_params": ["content", "hint", "injection_scale", "tags", "significance"]}),
                    json!({"name": "remember", "one_liner": "Create entities/relationships/observations in the KG", "key_params": ["kind", "data", "confidence", "source_thought_id"]}),
                    json!({"name": "search", "one_liner": "Unified LM search: memories (default) + optional thoughts", "key_params": ["query", "target", "include_thoughts", "top_k_memories", "top_k_thoughts"]}),
                    json!({"name": "maintain", "one_liner": "Archival, export, re-embed checks and housekeeping", "key_params": ["subcommand", "limit", "dry_run", "output_dir"]}),
                    json!({"name": "call_gem", "one_liner": "Delegate a prompt to the Gemini CLI agent", "key_params": ["prompt", "model", "cwd", "mode"]}),
                    json!({"name": "call_cc", "one_liner": "Delegate a prompt to the Claude Code CLI agent", "key_params": ["prompt", "model", "cwd", "mode"]}),
                    json!({"name": "call_codex", "one_liner": "Delegate a prompt to the Codex CLI agent", "key_params": ["prompt", "model", "cwd", "mode"]}),
                    json!({"name": "call_status", "one_liner": "Get status of an async agent job", "key_params": ["job_id"]}),
                    json!({"name": "call_jobs", "one_liner": "List async agent jobs", "key_params": ["limit", "status_filter", "tool_name"]}),
                    json!({"name": "call_cancel", "one_liner": "Cancel a running or queued job", "key_params": ["job_id"]}),
                    json!({"name": "howto", "one_liner": "Get help for a specific tool or list all tools", "key_params": ["tool", "format"]}),
                    json!({"name": "wander", "one_liner": "Explore the knowledge graph for curiosity-driven discovery", "key_params": ["mode", "current_thought_id", "visited_ids", "recency_bias", "for"]}),
                    json!({"name": "rethink", "one_liner": "Revise or mark knowledge graph items for correction", "key_params": ["target_id", "mode", "mark_type", "marked_for"]}),
                    json!({"name": "corrections", "one_liner": "List recent correction events to inspect the learning journey", "key_params": ["target_id", "limit"]}),
                ];
                return Ok(CallToolResult::structured(json!({ "tools": tools })));
            }
            Some(t) => t,
        };

        let help = match tool {
            "think" => json!({
                "name": "think",
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
            "search" => json!({
                "name": "search",
                "description": "Unified search in LegacyMind: searches memories by default and, when include_thoughts=true, also searches thoughts. Supports continuity field filters for thoughts and forensic mode for provenance tracking.",
                "arguments": {
                    "query": "object — {name?, text?, id?} query parameters",
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
                    "order": "string? ('created_at_asc'|'created_at_desc') — order thoughts by created_at",
                    "forensic": "boolean — include correction chain and derivatives in results"
                },
                "returns": {"memories": {"items": "array"}, "thoughts": {"total": "number", "results": "array"}},
                "examples": [
                    {"description": "Search entities with forensic provenance", "call": {"query": {"name": "REMini"}, "target": "entity", "forensic": true}},
                    {"description": "Search thoughts in a specific session", "call": {"include_thoughts": true, "session_id": "session_123"}}
                ]
            }),
            "wander" => json!({
                "name": "wander",
                "description": "Interactively explore the knowledge graph via traversals. Can wander randomly, semantically, or via metadata and attention marks.",
                "arguments": {
                    "mode": "string (required) — 'random', 'semantic', 'meta', 'marks'",
                    "current_thought_id": "string — optional starting thought ID",
                    "visited_ids": "array — IDs to avoid preventing loops",
                    "recency_bias": "boolean (default false) — prioritize recent memories",
                    "for": "string — filter marks assigned to a specific member ('cc', 'sam', 'gemini', 'dt', 'gem')"
                },
                "returns": {
                    "current_node": "object — the node reached in the step",
                    "mode_used": "string — the mode used for the step",
                    "affordances": "array — suggested next modes",
                    "guidance": "string — actionable architectural guidance",
                    "queue_depth": "integer? — remaining items in queue (marks mode only)"
                },
                "examples": [
                    {"description": "Surface marks for CC", "call": {"mode": "marks", "for": "cc"}},
                    {"description": "Wander semantically from a specific thought", "call": {"mode": "semantic", "current_thought_id": "thoughts:abc"}}
                ]
            }),
            "rethink" => json!({
                "name": "rethink",
                "description": "Revise or mark knowledge graph items for correction. Supports provenance-tracked corrections and attention routing.",
                "arguments": {
                    "target_id": "string (required) — ID of the record (thoughts:xxx, entity:xxx, observation:xxx)",
                    "mode": "string (required) — 'mark' (flag for review) or 'correct' (apply fix)",
                    "mark_type": "string — 'correction', 'research', 'enrich', 'expand' (mark mode)",
                    "marked_for": "string — 'cc', 'sam', 'gemini', 'dt', 'gem' (mark mode)",
                    "note": "string — contextual explanation for the mark (mark mode)",
                    "reasoning": "string — why the record is being corrected (correct mode)",
                    "sources": "string[] — verification sources (correct mode)",
                    "cascade": "boolean (default false) — flag derivatives for review (correct mode)"
                },
                "returns": {
                    "success": "boolean",
                    "marked": "object? — details of the created mark",
                    "correction": "object? — details of the applied correction event",
                    "derivatives_flagged": "integer? — count of cascaded marks"
                }
            }),
            "corrections" => json!({
                "name": "corrections",
                "description": "List recent correction events to inspect the learning journey of the KG.",
                "arguments": {
                    "target_id": "string — optional filter for a specific target ID",
                    "limit": "integer (default 10) — max events to return"
                },
                "returns": {
                    "success": "boolean",
                    "count": "integer",
                    "events": "array of correction_event objects"
                }
            }),
            "remember" => json!({
                "name": "remember",
                "description": "Create personal memory entities or relationships; returns created id.",
                "arguments": {
                    "kind": "string — 'entity'|'relationship'|'observation'",
                    "data": "object — entity: {name, entity_type?, properties?} | relationship: {source, target, rel_type, properties?} | observation: {source, observation_type, properties?}",
                    "confidence": "number — optional confidence",
                    "upsert": "boolean (default true) — whether to find existing matching record or always create new"
                },
                "returns": {"created": true, "id": "string", "kind": "string"}
            }),
            "maintain" => json!({
                "name": "maintain",
                "description": "Maintenance operations including archival, cleanup, embedding refresh, rethink queue processing, and health checks (thoughts/entities/observations/edges).",
                "arguments": {
                    "subcommand": "string (required) — 'list_removal_candidates'|'export_removals'|'finalize_removal'|'health_check_embeddings'|'health_check_indexes'|'reembed'|'reembed_kg'|'embed_pending'|'ensure_continuity_fields'|'echo_config'|'corrections'|'rethink'|'populate'|'embed'|'wander'|'health'|'report'|'tasks'",
                    "dry_run": "boolean (default: false) — simulate operation without changes",
                    "limit": "integer|string (default: 100) — max items to process",
                    "format": "string (default: 'json') — export format",
                    "output_dir": "string (default: './archive') — export directory",
                    "tasks": "string — comma separated list for subcommand 'tasks' (default populate,embed,rethink,wander,health,report,corrections)",
                    "target_id": "string — optional filter for 'corrections' subcommand",
                    "rethink_types": "string — comma-separated mark types for 'rethink' subcommand (e.g., correction,research)"
                },
                "returns": {
                    "health_check_embeddings": "object — detailed breakdown per table (total, ok, missing, mismatched) with sample IDs",
                    "corrections": "object — {success, count, events[]} result from corrections bridge",
                    "rethink/populate/embed/wander/health": "object — {task, success, stdout, stderr}",
                    "tasks": "object — {results: [...]} aggregated per task",
                    "report": "object — contents of logs/remini_report.json",
                    "embed_pending": "object — {message, processed, succeeded, failed, remaining, dry_run} — retry embedding for thoughts with pending/failed status",
                    "other_subcommands": "object — counts, paths, or messages depending on operation"
                }
            }),
            "call_gem" => json!({
                "name": "call_gem",
                "description": "Delegate a prompt to the Gemini CLI agent. Supports session resume and observe mode.",
                "arguments": {
                    "prompt": "string (required) — the prompt text",
                    "model": "string — override model (env: GEMINI_MODEL/GEMINI_MODELS)",
                    "cwd": "string (required) — working directory for the agent",
                    "resume_session_id": "string — resume a specific Gemini session",
                    "continue_latest": "boolean (default false) — resume last Gemini session",
                    "timeout_ms": "integer (default 60000) — outer timeout",
                    "tool_timeout_ms": "integer (default 300000) — per-tool timeout",
                    "expose_stream": "boolean — include stream events in response",
                    "mode": "string — 'execute' (default) or 'observe' (read-only analysis)",
                    "max_response_chars": "integer (default 100000) — max chars for response (0 = no limit)"
                },
                "returns": {"status": "completed", "session_id": "string", "response": "string"}
            }),
            "call_cc" => json!({
                "name": "call_cc",
                "description": "Delegate a prompt to the Claude Code CLI agent. Supports session resume and observe mode.",
                "arguments": {
                    "prompt": "string (required) — the prompt text",
                    "model": "string — override model (env: ANTHROPIC_MODEL/ANTHROPIC_MODELS)",
                    "cwd": "string (required) — working directory for the agent",
                    "resume_session_id": "string — resume a specific Claude session",
                    "continue_latest": "boolean (default false) — resume last Claude session",
                    "timeout_ms": "integer (default 60000) — outer timeout",
                    "tool_timeout_ms": "integer (default 300000) — per-tool timeout",
                    "expose_stream": "boolean — include stream events in metadata",
                    "mode": "string — 'execute' (default) or 'observe' (read-only analysis)",
                    "max_response_chars": "integer (default 100000) — max chars for response (0 = no limit)"
                },
                "returns": {"status": "completed", "session_id": "string", "response": "string"}
            }),
            "call_codex" => json!({
                "name": "call_codex",
                "description": "Delegate a prompt to the Codex CLI agent. Supports session resume and observe mode.",
                "arguments": {
                    "prompt": "string (required) — the prompt text",
                    "model": "string — override model (env: CODEX_MODEL/CODEX_MODELS)",
                    "cwd": "string (required) — working directory for the agent",
                    "resume_session_id": "string — resume a specific Codex session",
                    "continue_latest": "boolean (default false) — resume last Codex session",
                    "timeout_ms": "integer (default 60000) — outer timeout",
                    "tool_timeout_ms": "integer (default 300000) — per-tool timeout",
                    "expose_stream": "boolean — include stream events in metadata",
                    "fire_and_forget": "boolean (default false) — enqueue without waiting",
                    "mode": "string — 'execute' (default) or 'observe' (read-only analysis)",
                    "max_response_chars": "integer (default 100000) — max chars for response (0 = no limit)"
                },
                "returns": {"status": "completed", "session_id": "string", "response": "string"}
            }),

            "call_status" => json!({
                "name": "call_status",
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
            "call_jobs" => json!({
                "name": "call_jobs",
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
            "call_cancel" => json!({
                "name": "call_cancel",
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
