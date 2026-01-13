//! maintenance_ops tool handler for archival and cleanup operations

use crate::error::{Result, SurrealMindError};
use crate::indexes::{IndexHealth, TableInfo, get_expected_indexes};
use crate::server::SurrealMindServer;
// corrections tool handler is in scope via SurrealMindServer impl; no direct import needed
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Parameters for the maintenance_ops tool
#[derive(Debug, serde::Deserialize)]
pub struct MaintenanceParams {
    pub subcommand: String,
    #[serde(default)]
    pub dry_run: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u64_forgiving"
    )]
    pub limit: Option<u64>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default)]
    pub tasks: Option<String>,
    #[serde(default)]
    pub target_id: Option<String>,
    #[serde(default)]
    pub rethink_types: Option<String>,
}

impl SurrealMindServer {
    /// Handle health check for database indexes
    async fn handle_health_check_indexes(&self, _dry_run: bool) -> Result<CallToolResult> {
        let mut results = vec![];

        for table_def in get_expected_indexes() {
            // Get current indexes for table
            let info: Vec<TableInfo> = self
                .db
                .query(format!("INFO FOR TABLE {}", table_def.table))
                .await?
                .take(0)?;

            let table_info = info.first().ok_or_else(|| SurrealMindError::Internal {
                message: format!("No info returned for table {}", table_def.table),
            })?;

            // Get expected index names (both required and optional)
            let mut expected = table_def
                .required
                .iter()
                .map(|idx| idx.to_definition().replace("{table}", &table_def.table))
                .collect::<Vec<_>>();
            let optional = table_def
                .optional
                .iter()
                .map(|idx| idx.to_definition().replace("{table}", &table_def.table))
                .collect::<Vec<_>>();
            expected.extend(optional);

            // Get present indexes
            let present = table_info.indexes.values().cloned().collect::<Vec<_>>();

            // Calculate missing (required only)
            let required_defs = table_def
                .required
                .iter()
                .map(|idx| idx.to_definition().replace("{table}", &table_def.table))
                .collect::<Vec<_>>();
            let missing = required_defs
                .iter()
                .filter(|req| !present.contains(req))
                .cloned()
                .collect::<Vec<_>>();

            results.push(IndexHealth {
                table: table_def.table.clone(),
                expected,
                present,
                missing,
            });
        }

        // Group by table for cleaner output
        let report = json!({
            "tables": results.iter().map(|r| json!({
                "table": r.table,
                "expected": r.expected,
                "present": r.present,
                "missing": r.missing,
                "status": if r.missing.is_empty() { "ok" } else { "missing_required" }
            })).collect::<Vec<_>>()
        });

        Ok(CallToolResult::structured(report))
    }
    /// Handle the maintenance_ops tool call
    pub async fn handle_maintenance_ops(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: MaintenanceParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
            })?;

        let dry_run = params.dry_run.unwrap_or(false);
        let limit = params.limit.unwrap_or(100) as usize;
        let format = params.format.unwrap_or_else(|| "json".to_string());
        let output_dir = params.output_dir.unwrap_or_else(|| "./archive".to_string());

        tracing::info!(
            "maintenance_ops called: subcommand={}, dry_run={}, limit={}, format={}, output_dir={}",
            params.subcommand,
            dry_run,
            limit,
            format,
            output_dir
        );

        match params.subcommand.as_str() {
            "list_removal_candidates" => self.handle_list_removal_candidates(limit, dry_run).await,
            "export_removals" => {
                self.handle_export_removals(limit, &format, &output_dir, dry_run)
                    .await
            }
            "finalize_removal" => self.handle_finalize_removal(limit, dry_run).await,
            "health_check_embeddings" => self.handle_health_check_embeddings(dry_run).await,
            "health_check_indexes" => self.handle_health_check_indexes(dry_run).await,
            "reembed" => self.handle_reembed(limit, dry_run).await,
            "reembed_kg" => self.handle_reembed_kg(limit, dry_run).await,
            "embed_pending" => self.handle_embed_pending(limit, dry_run).await,
            "ensure_continuity_fields" => self.handle_ensure_continuity_fields(dry_run).await,
            "echo_config" => self.handle_echo_config().await,
            "corrections" => {
                self.handle_corrections_bridge(limit, params.target_id.clone())
                    .await
            }
            "rethink" => {
                let mut envs: Vec<(String, String)> = vec![];
                if let Some(rt) = params.rethink_types.clone() {
                    envs.push(("RETHINK_TYPES".into(), rt));
                }
                self.handle_spawn_binary("gem_rethink", dry_run, &envs)
                    .await
            }
            "populate" => {
                self.handle_spawn_binary("kg_populate", dry_run, &Vec::new())
                    .await
            }
            "embed" => {
                self.handle_spawn_binary("kg_embed", dry_run, &Vec::new())
                    .await
            }
            "wander" => {
                self.handle_spawn_binary("kg_wander", dry_run, &Vec::new())
                    .await
            }
            "health" => {
                self.handle_spawn_script("scripts/sm_health.sh", dry_run)
                    .await
            }
            "report" => self.handle_report().await,
            "tasks" => self.handle_tasks(params.tasks.clone(), dry_run).await,
            _ => Err(SurrealMindError::Validation {
                message: format!("Unknown subcommand: {}", params.subcommand),
            }),
        }
    }

    async fn handle_tasks(&self, tasks: Option<String>, dry_run: bool) -> Result<CallToolResult> {
        let default_tasks: Vec<String> = vec![
            "populate".into(),
            "embed".into(),
            "rethink".into(),
            "wander".into(),
            "health".into(),
            "report".into(),
            "corrections".into(),
        ];
        let list = tasks.unwrap_or_else(|| default_tasks.join(","));
        let mut tasks_vec: Vec<String> = list
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if tasks_vec.iter().any(|t| t == "all") {
            tasks_vec = default_tasks.clone();
        }
        if tasks_vec.is_empty() {
            tasks_vec = default_tasks;
        }
        let mut results: Vec<serde_json::Value> = Vec::new();
        for t in tasks_vec {
            let res = match t.as_str() {
                "corrections" => self.handle_corrections_bridge(100, None).await,
                "rethink" => {
                    let envs: Vec<(String, String)> = Vec::new();
                    self.handle_spawn_binary("gem_rethink", dry_run, &envs)
                        .await
                }
                "populate" => {
                    let envs: Vec<(String, String)> = Vec::new();
                    self.handle_spawn_binary("kg_populate", dry_run, &envs)
                        .await
                }
                "embed" => {
                    let envs: Vec<(String, String)> = Vec::new();
                    self.handle_spawn_binary("kg_embed", dry_run, &envs).await
                }
                "wander" => {
                    let envs: Vec<(String, String)> = Vec::new();
                    self.handle_spawn_binary("kg_wander", dry_run, &envs).await
                }
                "health" => {
                    self.handle_spawn_script("scripts/sm_health.sh", dry_run)
                        .await
                }
                "report" => self.handle_report().await,
                other => Err(SurrealMindError::Validation {
                    message: format!("Unknown task in list: {}", other),
                }),
            };
            match res {
                Ok(r) => {
                    let payload = r
                        .structured_content
                        .clone()
                        .unwrap_or_else(|| json!(r.content));
                    results.push(payload);
                }
                Err(e) => results.push(json!({ "error": e.to_string(), "task": t })),
            }
        }
        Ok(CallToolResult::structured(json!({ "results": results })))
    }

    async fn handle_corrections_bridge(
        &self,
        limit: usize,
        target_id: Option<String>,
    ) -> Result<CallToolResult> {
        let mut map = serde_json::Map::new();
        map.insert("limit".into(), json!(limit as i64));
        if let Some(tid) = target_id {
            map.insert("target_id".into(), json!(tid));
        }
        let req = CallToolRequestParam {
            name: "corrections".into(),
            arguments: Some(map),
        };
        self.handle_corrections(req).await
    }

    async fn handle_spawn_binary(
        &self,
        bin: &str,
        dry_run: bool,
        extra_env: &[(String, String)],
    ) -> Result<CallToolResult> {
        let bin_path = format!("{}/target/release/{}", env!("CARGO_MANIFEST_DIR"), bin);
        let mut cmd = Command::new(bin_path);
        if dry_run {
            cmd.env("DRY_RUN", "1");
        }
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        let output = cmd.output().map_err(|e| SurrealMindError::Internal {
            message: format!("failed to run {}: {}", bin, e),
        })?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let success = output.status.success();
        let report = json!({
            "task": bin,
            "success": success,
            "stdout": stdout,
            "stderr": stderr
        });
        Ok(CallToolResult::structured(report))
    }

    async fn handle_spawn_script(&self, script: &str, dry_run: bool) -> Result<CallToolResult> {
        let script_path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), script);
        let mut cmd = Command::new("bash");
        cmd.arg(script_path);
        if dry_run {
            cmd.env("DRY_RUN", "1");
        }
        let output = cmd.output().map_err(|e| SurrealMindError::Internal {
            message: format!("failed to run script {}: {}", script, e),
        })?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let success = output.status.success();
        let report = json!({
            "task": script,
            "success": success,
            "stdout": stdout,
            "stderr": stderr
        });
        Ok(CallToolResult::structured(report))
    }

    async fn handle_report(&self) -> Result<CallToolResult> {
        let path = format!("{}/logs/remini_report.json", env!("CARGO_MANIFEST_DIR"));
        let contents = fs::read_to_string(&path).unwrap_or_else(|_| "{}".to_string());
        let json_val: serde_json::Value =
            serde_json::from_str(&contents).unwrap_or(json!({"warning": "report not found"}));
        Ok(CallToolResult::structured(json_val))
    }

    /// Return effective runtime configuration (safe subset) for debugging client/DB mismatch
    async fn handle_echo_config(&self) -> Result<CallToolResult> {
        let (prov, model, dim) = self.get_embedding_metadata();
        let rt = &self.config.runtime;
        let sys = &self.config.system;
        let out = json!({
            "db": {"url": sys.database_url, "ns": sys.database_ns, "db": sys.database_db},
            "embedding": {"provider": prov, "model": model, "dim": dim},
            "transport": rt.transport,
            "http": {"bind": rt.http_bind.to_string(), "path": rt.http_path},
            "mcp_no_log": rt.mcp_no_log,
        });
        Ok(CallToolResult::structured(out))
    }

    /// Ensure continuity fields and indexes exist on thoughts table
    async fn handle_ensure_continuity_fields(&self, dry_run: bool) -> Result<CallToolResult> {
        let mut created_fields = vec![];
        let mut created_indexes = vec![];
        let mut existing_fields = vec![];
        let mut existing_indexes = vec![];

        // Fields to ensure exist (SurrealDB 2.x type syntax)
        // Use option<...> instead of "NULL" suffix and record<thoughts> for record types.
        let fields: Vec<(&str, &str)> = vec![
            ("session_id", "option<string>"),
            ("chain_id", "option<string>"),
            ("previous_thought_id", "option<record<thoughts> | string>"),
            ("revises_thought", "option<record<thoughts> | string>"),
            ("branch_from", "option<record<thoughts> | string>"),
            ("confidence", "option<float>"),
        ];

        // Indexes to ensure exist
        let indexes = vec![
            "idx_thoughts_session: session_id, created_at",
            "idx_thoughts_chain: chain_id, created_at",
        ];

        let fields_len = fields.len();
        let indexes_len = indexes.len();

        // Check and create fields
        for (field_name, field_type) in &fields {
            let full_field_def = format!(
                "DEFINE FIELD {} ON TABLE thoughts TYPE {}",
                field_name, field_type
            );

            // Check if field exists (simple check - may not catch all cases)
            let check_query = "INFO FOR TABLE thoughts".to_string();
            if let Ok(mut response) = self.db.query(&check_query).await
                && let Ok(vec) = response.take::<Vec<serde_json::Value>>(0)
                && let Some(table_info) = vec.first()
                && let Some(fields_obj) = table_info.get("fields")
                && fields_obj.get(field_name).is_some()
            {
                existing_fields.push((*field_name).to_string());
                continue;
            }

            if !dry_run {
                match self.db.query(&full_field_def).await {
                    Ok(_) => {
                        created_fields.push((*field_name).to_string());
                        tracing::info!("Created continuity field: {}", field_name);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create continuity field {}: {}", field_name, e);
                        // Continue with other fields
                    }
                }
            } else {
                created_fields.push(format!("{} (dry-run)", field_name));
            }
        }

        // Check and create indexes
        for index_def in &indexes {
            let parts: Vec<&str> = index_def.split(": ").collect();
            if parts.len() != 2 {
                continue;
            }
            let index_name = parts[0];
            let index_cols = parts[1];
            let full_index_def = format!(
                "DEFINE INDEX {} ON TABLE thoughts FIELDS {};",
                index_name, index_cols
            );

            // Check if index exists (simple check - may not catch all cases)
            let check_query = "INFO FOR TABLE thoughts".to_string();
            if let Ok(mut response) = self.db.query(&check_query).await
                && let Ok(vec) = response.take::<Vec<serde_json::Value>>(0)
                && let Some(table_info) = vec.first()
                && let Some(indexes_obj) = table_info.get("indexes")
                && indexes_obj.get(index_name).is_some()
            {
                existing_indexes.push(index_name.to_string());
                continue;
            }

            if !dry_run {
                match self.db.query(&full_index_def).await {
                    Ok(_) => {
                        created_indexes.push(index_name.to_string());
                        tracing::info!("Created continuity index: {}", index_name);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create continuity index {}: {}", index_name, e);
                        // Continue with other indexes
                    }
                }
            } else {
                created_indexes.push(format!("{} (dry-run)", index_name));
            }
        }

        let result = json!({
            "created_fields": created_fields,
            "created_indexes": created_indexes,
            "existing_fields": existing_fields,
            "existing_indexes": existing_indexes,
            "dry_run": dry_run,
            "summary": format!(
                "Fields: {}/{} created, {}/{} existing. Indexes: {}/{} created, {}/{} existing.",
                created_fields.len(),
                fields_len,
                existing_fields.len(),
                fields_len,
                created_indexes.len(),
                indexes_len,
                existing_indexes.len(),
                indexes_len
            )
        });

        Ok(CallToolResult::structured(result))
    }

    async fn handle_health_check_embeddings(&self, _dry_run: bool) -> Result<CallToolResult> {
        // Determine expected embedding dimension from active embedder
        let expected = self.embedder.dimensions() as i64;
        let tables = vec!["thoughts", "kg_entities", "kg_observations", "kg_edges"];
        let mut report = serde_json::Map::new();

        report.insert("expected_dim".to_string(), json!(expected));

        for table in tables {
            // 1. Total count
            let q_total = format!("SELECT count() AS c FROM {} GROUP ALL", table);
            let total_res: Vec<serde_json::Value> = self.db.query(&q_total).await?.take(0)?;
            let total = total_res
                .first()
                .and_then(|v| v.get("c"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            // 2. OK count (valid array of correct length)
            let q_ok = format!(
                "SELECT count() AS c FROM {} WHERE type::is::array(embedding) AND array::len(embedding) = $d GROUP ALL",
                table
            );
            let ok_res: Vec<serde_json::Value> =
                self.db.query(&q_ok).bind(("d", expected)).await?.take(0)?;
            let ok = ok_res
                .first()
                .and_then(|v| v.get("c"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            // 3. Missing count (NONE or NULL)
            let q_missing = format!(
                "SELECT count() AS c FROM {} WHERE embedding IS NONE OR embedding = NULL GROUP ALL",
                table
            );
            let missing_res: Vec<serde_json::Value> = self.db.query(&q_missing).await?.take(0)?;
            let missing = missing_res
                .first()
                .and_then(|v| v.get("c"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            // 4. Mismatched count (Array but wrong length)
            let q_mismatched = format!(
                "SELECT count() AS c FROM {} WHERE type::is::array(embedding) AND array::len(embedding) != $d GROUP ALL",
                table
            );
            let mismatched_res: Vec<serde_json::Value> = self
                .db
                .query(&q_mismatched)
                .bind(("d", expected))
                .await?
                .take(0)?;
            let mismatched = mismatched_res
                .first()
                .and_then(|v| v.get("c"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            // 5. Get sample IDs for missing
            let q_sample_missing = format!(
                "SELECT meta::id(id) as id FROM {} WHERE embedding IS NONE OR embedding = NULL LIMIT 5",
                table
            );
            let sample_missing_res: Vec<serde_json::Value> =
                self.db.query(&q_sample_missing).await?.take(0)?;
            let sample_missing: Vec<String> = sample_missing_res
                .iter()
                .filter_map(|v| v.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()))
                .collect();

            // 6. Get sample IDs for mismatched
            let q_sample_mismatched = format!(
                "SELECT meta::id(id) as id FROM {} WHERE type::is::array(embedding) AND array::len(embedding) != $d LIMIT 5",
                table
            );
            let sample_mismatched_res: Vec<serde_json::Value> = self
                .db
                .query(&q_sample_mismatched)
                .bind(("d", expected))
                .await?
                .take(0)?;
            let sample_mismatched: Vec<String> = sample_mismatched_res
                .iter()
                .filter_map(|v| v.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()))
                .collect();

            let table_stats = json!({
                "total": total,
                "ok": ok,
                "missing": {
                    "count": missing,
                    "samples": sample_missing
                },
                "mismatched_dim": {
                    "count": mismatched,
                    "samples": sample_mismatched
                },
                "unknown_state": total.saturating_sub(ok + missing + mismatched) // Should be 0 if coverage is complete
            });

            report.insert(table.to_string(), table_stats);
        }

        // Add pending embeddings count (graceful degradation feature)
        let pending_query = r#"
            SELECT count() AS c FROM thoughts 
            WHERE embedding_status IN ['pending', 'failed'] 
            GROUP ALL
        "#;
        let pending_res: Vec<serde_json::Value> = self.db.query(pending_query).await?.take(0)?;
        let pending_count = pending_res
            .first()
            .and_then(|v| v.get("c"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        report.insert(
            "pending_embeddings".to_string(),
            json!({
                "count": pending_count,
                "note": "Use 'maintain embed_pending' to retry these"
            }),
        );

        Ok(CallToolResult::structured(serde_json::Value::Object(
            report,
        )))
    }

    async fn handle_list_removal_candidates(
        &self,
        limit: usize,
        dry_run: bool,
    ) -> Result<CallToolResult> {
        tracing::info!("Listing removal candidates (dry_run={})", dry_run);

        let retention_days = std::env::var("SURR_RETENTION_DAYS")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(30);

        // No need for cutoff, use time::now() directly in query

        let query = format!(
            "SELECT meta::id(id) as id, content, created_at FROM thoughts WHERE status = 'removal' AND created_at < time::now() - {}d LIMIT {}",
            retention_days, limit
        );

        let candidates: Vec<serde_json::Value> = self.db.query(&query).await?.take(0)?;

        let summary = json!({
            "total_candidates": candidates.len(),
            "retention_days": retention_days,
            "dry_run": dry_run,
            "candidates": candidates.into_iter().map(|c| {
                let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let content_preview = c.get("content").and_then(|v| v.as_str()).unwrap_or("").chars().take(100).collect::<String>();
                json!({
                    "id": id,
                    "content_preview": content_preview,
                    "created_at": c.get("created_at")
                })
            }).collect::<Vec<_>>()
        });

        Ok(CallToolResult::structured(summary))
    }

    async fn handle_export_removals(
        &self,
        limit: usize,
        format: &str,
        output_dir: &str,
        dry_run: bool,
    ) -> Result<CallToolResult> {
        tracing::info!(
            "Exporting removals (dry_run={}, format={}, output_dir={})",
            dry_run,
            format,
            output_dir
        );

        if format != "json" {
            return Err(SurrealMindError::Validation {
                message: format!("Unsupported format: {}. Only 'json' is supported.", format),
            });
        }

        // Get candidates
        let retention_days = std::env::var("SURR_RETENTION_DAYS")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(30);

        let query = format!(
            "SELECT * FROM thoughts WHERE status = 'removal' AND created_at < time::now() - {}d LIMIT {}",
            retention_days, limit
        );

        let thoughts: Vec<serde_json::Value> = self.db.query(&query).await?.take(0)?;

        if thoughts.is_empty() {
            let summary = json!({
                "exported_count": 0,
                "file_path": null,
                "dry_run": dry_run,
                "message": "No thoughts to export"
            });
            return Ok(CallToolResult::structured(summary));
        }

        // Ensure output dir exists
        if !dry_run {
            fs::create_dir_all(output_dir).map_err(|e| SurrealMindError::Internal {
                message: format!("Failed to create output directory: {}", e),
            })?;
        }

        // Generate file path
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("thoughts_removal_{}.json", timestamp);
        let file_path = Path::new(output_dir).join(filename);

        // Serialize to JSON
        let json_data = serde_json::to_string_pretty(&thoughts).map_err(|e| {
            SurrealMindError::Serialization {
                message: format!("Failed to serialize thoughts: {}", e),
            }
        })?;

        if !dry_run {
            fs::write(&file_path, json_data).map_err(|e| SurrealMindError::Internal {
                message: format!("Failed to write export file: {}", e),
            })?;
        }

        let summary = json!({
            "exported_count": thoughts.len(),
            "file_path": file_path.to_string_lossy(),
            "dry_run": dry_run,
            "retention_days": retention_days
        });

        Ok(CallToolResult::structured(summary))
    }

    async fn handle_finalize_removal(&self, limit: usize, dry_run: bool) -> Result<CallToolResult> {
        tracing::info!("Finalizing removals (dry_run={})", dry_run);

        let retention_days = std::env::var("SURR_RETENTION_DAYS")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(30);

        let query = format!(
            "SELECT meta::id(id) as id FROM thoughts WHERE status = 'removal' AND created_at < time::now() - {}d LIMIT {}",
            retention_days, limit
        );

        let candidates: Vec<serde_json::Value> = self.db.query(&query).await?.take(0)?;

        if candidates.is_empty() {
            let summary = json!({
                "deleted_count": 0,
                "dry_run": dry_run,
                "message": "No thoughts to delete"
            });
            return Ok(CallToolResult::structured(summary));
        }

        let ids: Vec<String> = candidates
            .into_iter()
            .filter_map(|c| c.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();

        let deleted_count = ids.len();

        if !dry_run {
            let delete_query = "DELETE FROM thoughts WHERE id IN $ids";
            self.db.query(delete_query).bind(("ids", ids)).await?;
        }

        let summary = json!({
            "deleted_count": deleted_count,
            "dry_run": dry_run,
            "retention_days": retention_days
        });

        Ok(CallToolResult::structured(summary))
    }

    async fn handle_reembed(&self, limit: usize, dry_run: bool) -> Result<CallToolResult> {
        // Call the reembed function from lib.rs
        let batch_size = 100; // Default batch size
        let stats = crate::run_reembed(batch_size, Some(limit), false, dry_run).await?;
        let result = json!({
            "expected_dim": stats.expected_dim,
            "batch_size": stats.batch_size,
            "processed": stats.processed,
            "updated": stats.updated,
            "skipped": stats.skipped,
            "missing": stats.missing,
            "mismatched": stats.mismatched,
            "dry_run": dry_run
        });
        Ok(CallToolResult::structured(result))
    }

    async fn handle_reembed_kg(&self, limit: usize, dry_run: bool) -> Result<CallToolResult> {
        // Call the library function directly
        let limit_opt = if limit == 0 { None } else { Some(limit) };
        let stats = crate::run_reembed_kg(limit_opt, dry_run)
            .await
            .map_err(|e| SurrealMindError::Internal {
                message: format!("KG reembed failed: {}", e),
            })?;

        let result = json!({
            "message": "KG reembed completed",
            "expected_dim": stats.expected_dim,
            "provider": stats.provider,
            "model": stats.model,
            "entities": {
                "updated": stats.entities_updated,
                "skipped": stats.entities_skipped,
                "missing": stats.entities_missing,
                "mismatched": stats.entities_mismatched
            },
            "observations": {
                "updated": stats.observations_updated,
                "skipped": stats.observations_skipped,
                "missing": stats.observations_missing,
                "mismatched": stats.observations_mismatched
            },
            "dry_run": dry_run
        });
        Ok(CallToolResult::structured(result))
    }

    /// Handle embed_pending: retry embedding for thoughts with pending/failed status
    async fn handle_embed_pending(&self, limit: usize, dry_run: bool) -> Result<CallToolResult> {
        let limit_val = if limit == 0 { 100 } else { limit };

        // Query thoughts with pending or failed embedding status
        // Note: SurrealDB 2.4+ requires ORDER BY fields in SELECT clause
        let query = r#"
            SELECT meta::id(id) AS id, content, created_at 
            FROM thoughts 
            WHERE embedding_status IN ['pending', 'failed'] 
            ORDER BY created_at ASC 
            LIMIT $limit;
        "#;

        let mut response = self
            .db
            .query(query)
            .bind(("limit", limit_val as i64))
            .await?;

        let rows: Vec<serde_json::Value> = response.take(0)?;

        if rows.is_empty() {
            return Ok(CallToolResult::structured(json!({
                "message": "No pending embeddings found",
                "processed": 0,
                "succeeded": 0,
                "failed": 0,
                "remaining": 0
            })));
        }

        let mut processed = 0;
        let mut succeeded = 0;
        let mut failed = 0;

        for row in &rows {
            let id = row
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let content = row
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            if id.is_empty() || content.is_empty() {
                continue;
            }

            processed += 1;

            if dry_run {
                tracing::info!(thought_id = %id, "Would embed (dry run)");
                succeeded += 1;
                continue;
            }

            // Attempt embedding
            match self.embedder.embed(&content).await {
                Ok(embedding) if !embedding.is_empty() => {
                    // Update thought with embedding
                    let update_query = r#"
                        UPDATE type::thing('thoughts', $id) SET 
                        embedding = $embedding,
                        embedded_at = time::now(),
                        embedding_status = 'complete'
                        RETURN NONE;
                    "#;

                    if let Err(e) = self
                        .db
                        .query(update_query)
                        .bind(("id", id.clone()))
                        .bind(("embedding", embedding))
                        .await
                    {
                        tracing::warn!(thought_id = %id, error = %e, "Failed to update thought with embedding");
                        failed += 1;
                    } else {
                        tracing::info!(thought_id = %id, "Successfully embedded pending thought");
                        succeeded += 1;
                    }
                }
                Ok(_) => {
                    tracing::warn!(thought_id = %id, "Embedding returned empty vector");
                    failed += 1;
                }
                Err(e) => {
                    tracing::warn!(thought_id = %id, error = %e, "Embedding failed");
                    failed += 1;
                }
            }
        }

        // Count remaining pending
        let count_query = r#"
            SELECT count() AS cnt 
            FROM thoughts 
            WHERE embedding_status IN ['pending', 'failed'];
        "#;
        let mut count_response = self.db.query(count_query).await?;
        let count_rows: Vec<serde_json::Value> = count_response.take(0)?;
        let remaining = count_rows
            .first()
            .and_then(|r| r.get("cnt"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as usize;

        Ok(CallToolResult::structured(json!({
            "message": if dry_run { "Dry run complete" } else { "Embedding retry complete" },
            "processed": processed,
            "succeeded": succeeded,
            "failed": failed,
            "remaining": remaining.saturating_sub(succeeded),
            "dry_run": dry_run
        })))
    }
}
