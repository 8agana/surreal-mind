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

        let tool = match maybe_tool {
            None => {
                // Canonical tools roster
                let overview = json!([
                    {"name": "legacymind_think", "one_liner": "Unified thinking tool with automatic mode routing via triggers/heurs", "key_params": ["content", "hint", "injection_scale", "tags", "significance"]},
                    {"name": "photography_think", "one_liner": "Store photography thoughts with memory injection (isolated repo)", "key_params": ["content", "injection_scale", "tags", "significance"]},
                    {"name": "memories_create", "one_liner": "Create entities/relationships/observations in the KG", "key_params": ["kind", "data", "confidence", "source_thought_id"]},
                    {"name": "memories_moderate", "one_liner": "Review/decide on KG candidates", "key_params": ["action", "target", "status", "items", "dry_run"]},
                    {"name": "legacymind_search", "one_liner": "Unified LM search: memories (default) + optional thoughts", "key_params": ["query", "target", "include_thoughts", "top_k_memories", "top_k_thoughts"]},
                    {"name": "photography_search", "one_liner": "Unified Photography search: memories (default) + optional thoughts", "key_params": ["query", "target", "include_thoughts", "top_k_memories", "top_k_thoughts"]},
                    {"name": "maintenance_ops", "one_liner": "Archival, export, re-embed checks and housekeeping", "key_params": ["subcommand", "limit", "dry_run", "output_dir"]},
                    {"name": "inner_voice", "one_liner": "Retrieve relevant memories and thoughts with optional synthesis", "key_params": ["query", "top_k", "auto_extract_to_kg"]}
                ]);
                return Ok(CallToolResult::structured(overview));
            }
            Some(t) => t,
        };

        let help = match tool {
            "legacymind_think" => json!({
                "name": "legacymind_think",
                "description": "Unified thinking tool that routes to appropriate mode based on triggers, hint, or heuristics.",
                "arguments": {
                    "content": "string (required) — the thought text",
                    "hint": "string — optional explicit mode ('debug', 'build', 'plan', 'stuck', 'question', 'conclude')",
                    "injection_scale": "integer|string (0-3) — memory injection level (overrides mode default)",
                    "tags": "string[] — optional tags",
                    "significance": "number|string (0.0-1.0) — importance (overrides mode default)",
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
                    "contradiction_patterns": "string[] — optional custom patterns for contradiction detection (default: ['not', 'no', 'cannot', 'false', 'incorrect', 'fails', 'broken', 'doesn't', 'isn't', 'won't'])"
                },
                "returns": {
                    "mode_selected": "string",
                    "reason": "string",
                    "delegated_result": "object — result from the chosen mode",
                    "links": {
                        "session_id": "string? — resolved session identifier",
                        "chain_id": "string? — resolved chain identifier",
                        "previous_thought_id": "string? — resolved previous thought reference",
                        "revises_thought": "string? — resolved thought being revised",
                        "branch_from": "string? — resolved branch reference",
                        "confidence": "number? — clamped confidence value"
                    },
                    "verification": "object? — hypothesis verification result (only if needs_verification=true and hypothesis provided)",
                    "telemetry": "object — trigger/heuristic info + link resolution details"
                },
                "hypothesis_verification": {
                    "description": "Optional KG-based verification of a provided hypothesis. Embeds the hypothesis, searches similar KG entities/observations, classifies evidence as supporting/contradicting based on pattern matching, and computes a confidence score.",
                    "example": {
                        "input": {"hypothesis": "Rust is a memory-safe language", "needs_verification": true, "evidence_limit": 5},
                        "output": {"verification": {"hypothesis": "Rust is a memory-safe language", "supporting": [{"table": "kg_entities", "id": "kg_entities:123", "text": "Rust prevents memory errors", "similarity": 0.85, "provenance": {"entity_type": "language"}}], "contradicting": [], "confidence_score": 1.0, "suggested_revision": null, "telemetry": {"embedding_dim": 1536, "provider": "openai", "k": 100, "time_ms": 150}}}
                    },
                    "notes": ["Verification is deterministic and rule-based (no LLM calls)", "Results may include suggested revisions if confidence < 0.4", "Evidence is sorted by similarity and limited per bucket", "Default patterns detect common contradictions; customize with contradiction_patterns"]
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
            "photography_think" => json!({
                "name": "photography_think",
                "description": "Store photography thoughts with memory injection (isolated photography repo).",
                "arguments": {
                    "content": "string (required) — the thought text",
                    "injection_scale": "integer|string (0-3 or presets) — memory injection level (0=no injection, 1-3=scale)",
                    "tags": "string[] — optional tags",
                    "significance": "number|string (0.0-1.0 or presets) — importance"
                },
                "returns": {"thought_id": "string", "memories_injected": "number", "embedding_model": "string", "embedding_dim": "number", "framework_enhanced": "boolean"}
            }),
            "inner_voice" => json!({
                "name": "inner_voice",
                "description": "Retrieve relevant memories and thoughts with optional auto-extraction to KG candidates.",
                "arguments": {
                    "query": "string (required) — search query",
                    "top_k": "integer (1-50; default 10) — max snippets",
                    "sim_thresh": "number — similarity floor",
                    "floor": "number — minimum similarity",
                    "mix": "number (0.0-1.0; default 0.6) — KG/thoughts mix",
                    "include_private": "boolean (default false)",
                    "include_tags": "string[] — include thoughts with these tags",
                    "exclude_tags": "string[] — exclude thoughts with these tags",
                    "auto_extract_to_kg": "boolean (default false) — stage KG candidates"
                },
                "returns": {"snippets": "array", "answer": "string?", "diagnostics": "object"}
            }),
            "legacymind_search" => json!({
                "name": "legacymind_search",
                "description": "Unified search in LegacyMind: searches memories by default and, when include_thoughts=true, also searches thoughts. Supports continuity field filters for thoughts.",
                "arguments": {
                    "query": "object — used for memories search (e.g., {name})",
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
                    "kind": "string — 'entity'|'relationship'",
                    "data": "object — entity: {name, entity_type?, properties?} | relationship: {source, target, rel_type, properties?}",
                    "confidence": "number — optional confidence"
                },
                "returns": {"created": true, "id": "string", "kind": "string"}
            }),
            "photography_search" => json!({
                "name": "photography_search",
                "description": "Unified search in Photography (ns=photography, db=work): memories by default + optional thoughts when include_thoughts=true.",
                "arguments": {
                    "query": "object — used for memories (e.g., {name})",
                    "target": "'entity'|'relationship'|'observation'|'mixed' (default 'mixed')",
                    "include_thoughts": "boolean (default false)",
                    "thoughts_content": "string — optional explicit query text for thoughts",
                    "top_k_memories": "integer (1-50; default 10)",
                    "top_k_thoughts": "integer (1-50; default 5)",
                    "sim_thresh": "number (0.0-1.0) — similarity floor for thoughts"
                },
                "returns": {"memories": {"items": "array"}, "thoughts": {"total": "number", "results": "array"}}
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
            // Legacy aliases for KG help (kept as pointers only)
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
