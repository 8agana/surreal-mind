use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Map, Value};
use surrealdb::engine::remote::ws::Client as WsClient;
use surrealdb::Surreal;

use crate::clients::traits::{AgentError, AgentResponse, CognitiveAgent};
use crate::utils::db::upsert_tool_session;

pub struct PersistedAgent<A: CognitiveAgent> {
    agent: A,
    db: Arc<Surreal<WsClient>>,
    agent_source: String,
    agent_instance: String,
    tool_name: String,
    base_metadata: Map<String, Value>,
}

impl<A: CognitiveAgent> PersistedAgent<A> {
    pub fn new(
        agent: A,
        db: Arc<Surreal<WsClient>>,
        agent_source: impl Into<String>,
        agent_instance: impl Into<String>,
        tool_name: impl Into<String>,
    ) -> Self {
        Self {
            agent,
            db,
            agent_source: agent_source.into(),
            agent_instance: agent_instance.into(),
            tool_name: tool_name.into(),
            base_metadata: Map::new(),
        }
    }

    fn build_metadata(&self, resume_session: Option<&str>) -> Value {
        let mut metadata = self.base_metadata.clone();
        if let Some(resume) = resume_session {
            metadata.insert(
                "resume_session_id".to_string(),
                Value::String(resume.to_string()),
            );
        }
        Value::Object(metadata)
    }
}

#[async_trait]
impl<A: CognitiveAgent> CognitiveAgent for PersistedAgent<A> {
    async fn call(
        &self,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<AgentResponse, AgentError> {
        let response = self.agent.call(prompt, session_id).await?;
        let metadata = self.build_metadata(session_id);

        let sql = "CREATE agent_exchanges SET created_at = time::now(), agent_source = $source, agent_instance = $instance, prompt = $prompt, response = $response, tool_name = $tool, session_id = $session, metadata = $metadata RETURN meta::id(id) as id;";
        let created: Vec<Value> = self
            .db
            .query(sql)
            .bind(("source", self.agent_source.clone()))
            .bind(("instance", self.agent_instance.clone()))
            .bind(("prompt", prompt.to_string()))
            .bind(("response", response.response.clone()))
            .bind(("tool", self.tool_name.clone()))
            .bind(("session", response.session_id.clone()))
            .bind(("metadata", metadata))
            .await
            .map_err(|e| AgentError::CliError(format!("db insert failed: {}", e)))?
            .take(0)
            .map_err(|e| AgentError::CliError(format!("db response failed: {}", e)))?;

        let exchange_id = created
            .first()
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::ParseError("missing exchange id".to_string()))?;

        upsert_tool_session(
            self.db.as_ref(),
            self.tool_name.clone(),
            response.session_id.clone(),
            exchange_id.to_string(),
        )
        .await
        .map_err(|e| AgentError::CliError(format!("db tool session upsert failed: {}", e)))?;

        Ok(response)
    }
}
