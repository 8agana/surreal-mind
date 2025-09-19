//! Brain datastore tool handlers

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

impl SurrealMindServer {
    /// Handle the brain_store tool (get/set brain sections)
    pub async fn handle_brain_store(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        if !self.config.runtime.brain_enable {
            return Err(SurrealMindError::FeatureDisabled {
                message: "brain datastore is disabled (set SURR_ENABLE_BRAIN=1)".into(),
            });
        }

        let db = self
            .brain_db()
            .ok_or_else(|| SurrealMindError::FeatureDisabled {
                message: "brain datastore handle unavailable".into(),
            })?;

        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;

        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("get");
        let agent = args
            .get("agent")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .ok_or_else(|| SurrealMindError::InvalidParams {
                message: "Missing 'agent' parameter".into(),
            })?;
        let section = args
            .get("section")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .ok_or_else(|| SurrealMindError::InvalidParams {
                message: "Missing 'section' parameter".into(),
            })?;

        match action {
            "get" => {
                let mut result: Vec<serde_json::Value> = db
                    .query(
                        "SELECT agent, section, content, updated_at FROM brain_sections \
                         WHERE agent = $agent AND section = $section LIMIT 1",
                    )
                    .bind(("agent", agent.clone()))
                    .bind(("section", section.clone()))
                    .await?
                    .take(0)?;

                if let Some(entry) = result.pop() {
                    let response = json!({
                        "agent": agent,
                        "section": section,
                        "content": entry
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default(),
                        "updated_at": entry.get("updated_at").cloned(),
                        "found": true,
                    });
                    Ok(CallToolResult::structured(response))
                } else {
                    Ok(CallToolResult::structured(json!({
                        "agent": agent,
                        "section": section,
                        "content": "",
                        "updated_at": null,
                        "found": false,
                    })))
                }
            }
            "set" => {
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned())
                    .ok_or_else(|| SurrealMindError::InvalidParams {
                        message: "Missing 'content' parameter for set action".into(),
                    })?;

                // First attempt to update existing record
                let mut updated: Vec<serde_json::Value> = db
                    .query(
                        "UPDATE brain_sections SET content = $content, updated_at = time::now() \
                         WHERE agent = $agent AND section = $section RETURN agent, section, content, updated_at",
                    )
                    .bind(("agent", agent.clone()))
                    .bind(("section", section.clone()))
                    .bind(("content", content.clone()))
                    .await?
                    .take(0)?;

                if updated.is_empty() {
                    updated = db
                        .query(
                            "CREATE brain_sections SET agent = $agent, section = $section, content = $content, updated_at = time::now() \
                             RETURN agent, section, content, updated_at",
                        )
                        .bind(("agent", agent.clone()))
                        .bind(("section", section.clone()))
                        .bind(("content", content.clone()))
                        .await?
                        .take(0)?;
                }

                let entry = updated.first().cloned().unwrap_or_else(|| {
                    json!({
                        "agent": agent.clone(),
                        "section": section.clone(),
                        "content": content.clone()
                    })
                });

                let response = json!({
                    "agent": agent,
                    "section": section,
                    "content": entry
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or(content.as_str()),
                    "updated_at": entry.get("updated_at").cloned(),
                    "found": true,
                });

                Ok(CallToolResult::structured(response))
            }
            other => Err(SurrealMindError::InvalidParams {
                message: format!("Unsupported action '{}'. Expected 'get' or 'set'", other),
            }),
        }
    }
}
