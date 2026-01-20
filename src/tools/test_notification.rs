use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{
    CallToolRequestParam, CallToolResult, LoggingLevel, LoggingMessageNotificationParam,
};
use rmcp::service::{RequestContext, RoleServer};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct TestNotificationParams {
    pub message: String,
    #[serde(default)]
    pub level: Option<String>,
}

impl SurrealMindServer {
    /// Handle test_notification tool
    pub async fn handle_test_notification(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: TestNotificationParams =
            serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                SurrealMindError::InvalidParams {
                    message: format!("Invalid parameters: {}", e),
                }
            })?;

        let level = match params.level.as_deref() {
            Some("debug") => LoggingLevel::Debug,
            Some("info") => LoggingLevel::Info,
            Some("notice") => LoggingLevel::Notice,
            Some("warning") => LoggingLevel::Warning,
            Some("error") => LoggingLevel::Error,
            Some("critical") => LoggingLevel::Critical,
            Some("alert") => LoggingLevel::Alert,
            Some("emergency") => LoggingLevel::Emergency,
            _ => LoggingLevel::Info,
        };

        // Send notification via peer
        // Note: rmcp 0.6.4+ uses peer.notify_logging_message(param)
        let notification_param = LoggingMessageNotificationParam {
            level,
            logger: Some("surreal-mind".to_string()),
            data: serde_json::Value::String(params.message.clone()),
        };

        match context
            .peer
            .notify_logging_message(notification_param)
            .await
        {
            Ok(_) => Ok(CallToolResult::structured(json!({
                "status": "success",
                "message": format!("Notification sent: {}", params.message)
            }))),
            Err(e) => Err(SurrealMindError::Mcp {
                message: format!("Failed to send notification: {}", e),
            }),
        }
    }
}
