use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use chrono::Utc;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
pub struct LegacymindUpdateRequest {
    pub thought_id: String,
    pub updates: Map<String, Value>,
    #[serde(default)]
    pub reembed: Option<bool>,
}

fn normalize_tags(value: Value) -> Result<Value> {
    match value {
        Value::Null => Ok(Value::Null),
        Value::String(s) => Ok(Value::Array(vec![Value::String(s)])),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                if let Some(s) = item.as_str() {
                    out.push(Value::String(s.to_string()));
                } else {
                    return Err(SurrealMindError::InvalidParams {
                        message: "tags must be a string, array of strings, or null".into(),
                    });
                }
            }
            Ok(Value::Array(out))
        }
        _ => Err(SurrealMindError::InvalidParams {
            message: "tags must be a string, array of strings, or null".into(),
        }),
    }
}
impl SurrealMindServer {
    pub async fn handle_legacymind_update(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: LegacymindUpdateRequest = serde_json::from_value(Value::Object(args))
            .map_err(|e| SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
            })?;

        if params.updates.is_empty() {
            return Err(SurrealMindError::InvalidParams {
                message: "updates must include at least one field".into(),
            });
        }

        let allowed_fields: HashSet<&str> = [
            "content",
            "tags",
            "chain_id",
            "session_id",
            "previous_thought_id",
            "revises_thought",
            "branch_from",
            "extracted_to_kg",
            "extraction_batch_id",
            "extracted_at",
            "status",
            "significance",
            "injection_scale",
            "access_count",
            "last_accessed",
            "submode",
            "framework_enhanced",
            "framework_analysis",
            "origin",
            "is_private",
            "confidence",
        ]
        .into_iter()
        .collect();

        let mut update_map: Map<String, Value> = Map::new();
        let mut invalid_fields: Vec<String> = Vec::new();
        let mut fields_updated: HashSet<String> = HashSet::new();
        let mut content_for_reembed: Option<String> = None;
        let mut extracted_to_kg_set = false;
        let mut extracted_at_provided = false;

        for (key, value) in params.updates.into_iter() {
            if !allowed_fields.contains(key.as_str()) {
                invalid_fields.push(key);
                continue;
            }
            match key.as_str() {
                "content" => {
                    if let Some(s) = value.as_str() {
                        content_for_reembed = Some(s.to_string());
                        update_map.insert(key.clone(), Value::String(s.to_string()));
                        fields_updated.insert(key);
                    } else {
                        return Err(SurrealMindError::InvalidParams {
                            message: "content must be a string".into(),
                        });
                    }
                }
                "tags" => {
                    let normalized = normalize_tags(value)?;
                    update_map.insert(key.clone(), normalized);
                    fields_updated.insert(key);
                }
                "extracted_to_kg" => {
                    if let Some(b) = value.as_bool() {
                        update_map.insert(key.clone(), Value::Bool(b));
                        fields_updated.insert(key);
                        extracted_to_kg_set = b;
                    } else {
                        return Err(SurrealMindError::InvalidParams {
                            message: "extracted_to_kg must be a boolean".into(),
                        });
                    }
                }
                "extracted_at" => {
                    extracted_at_provided = true;
                    update_map.insert(key.clone(), value);
                    fields_updated.insert(key);
                }
                _ => {
                    update_map.insert(key.clone(), value);
                    fields_updated.insert(key);
                }
            }
        }

        if !invalid_fields.is_empty() {
            return Err(SurrealMindError::InvalidParams {
                message: format!("Unknown update fields: {}", invalid_fields.join(", ")),
            });
        }
        if update_map.is_empty() {
            return Err(SurrealMindError::InvalidParams {
                message: "No valid update fields provided".into(),
            });
        }

        if extracted_to_kg_set && !extracted_at_provided {
            update_map.insert(
                "extracted_at".to_string(),
                Value::String(Utc::now().to_rfc3339()),
            );
            fields_updated.insert("extracted_at".to_string());
        }

        let mut reembedded = false;
        if let Some(content) = content_for_reembed {
            if params.reembed.unwrap_or(true) {
                let embedding = self
                    .embedder
                    .embed(&content)
                    .await
                    .map_err(|e| SurrealMindError::Embedding {
                        message: e.to_string(),
                    })?;
                if embedding.is_empty() {
                    return Err(SurrealMindError::Embedding {
                        message: "Generated embedding is empty".into(),
                    });
                }
                let (provider, model, dim) = self.get_embedding_metadata();
                update_map.insert("embedding".to_string(), json!(embedding));
                update_map.insert("embedding_provider".to_string(), Value::String(provider));
                update_map.insert("embedding_model".to_string(), Value::String(model));
                update_map.insert("embedding_dim".to_string(), json!(dim));
                update_map.insert(
                    "embedded_at".to_string(),
                    Value::String(Utc::now().to_rfc3339()),
                );
                fields_updated.insert("embedding".to_string());
                fields_updated.insert("embedding_provider".to_string());
                fields_updated.insert("embedding_model".to_string());
                fields_updated.insert("embedding_dim".to_string());
                fields_updated.insert("embedded_at".to_string());
                reembedded = true;
            }
        }

        let raw_id = params
            .thought_id
            .strip_prefix("thoughts:")
            .unwrap_or(&params.thought_id);
        let update_value = Value::Object(update_map);
        let update_result = if let Ok(uuid_id) = Uuid::parse_str(raw_id) {
            self.db
                .update::<Option<Value>>(("thoughts", uuid_id))
                .merge(update_value)
                .await
        } else {
            self.db
                .update::<Option<Value>>(("thoughts", raw_id))
                .merge(update_value)
                .await
        };

        let updated = match update_result {
            Ok(record) => record.is_some(),
            Err(e) => {
                return Err(SurrealMindError::Database {
                    message: e.to_string(),
                })
            }
        };

        let mut fields: Vec<String> = fields_updated.into_iter().collect();
        fields.sort();

        Ok(CallToolResult::structured(json!({
            "thought_id": params.thought_id,
            "updated": updated,
            "fields_updated": fields,
            "reembedded": reembedded
        })))
    }
}
