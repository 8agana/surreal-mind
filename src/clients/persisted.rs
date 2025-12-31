use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Map, Value};
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client as WsClient;

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

#[derive(Debug, Deserialize)]
struct IdResult {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ExchangeRow {
    prompt: String,
    response: String,
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
        let context_sql = "SELECT prompt, response, created_at FROM agent_exchanges WHERE tool_name = $tool_name ORDER BY created_at ASC;";
        let mut context_response = self
            .db
            .query(context_sql)
            .bind(("tool_name", self.tool_name.clone()))
            .await
            .map_err(|e| AgentError::CliError(format!("db context query failed: {}", e)))?;
        let exchanges: Vec<ExchangeRow> = context_response
            .take::<Vec<ExchangeRow>>(0)
            .map_err(|e| AgentError::CliError(format!("db context response failed: {}", e)))?;

        let context = if exchanges.is_empty() {
            None
        } else {
            let mut assembled = String::new();
            for (idx, exchange) in exchanges.iter().enumerate() {
                assembled.push_str(&format!(
                    "Previous exchange {}:\nUser: {}\nAssistant: {}\n\n",
                    idx + 1,
                    exchange.prompt,
                    exchange.response
                ));
            }
            Some(assembled)
        };

        let prompt_to_send = if let Some(context) = context {
            format!("{context}\n\nCurrent question:\n{prompt}")
        } else {
            prompt.to_string()
        };

        let mut response = self.agent.call(&prompt_to_send, None).await?;
        let metadata = self.build_metadata(session_id);

        let sql = "CREATE agent_exchanges SET created_at = time::now(), agent_source = $arg_source, agent_instance = $instance, prompt = $prompt, response = $response, tool_name = $arg_tool, session_id = $arg_session, metadata = $metadata RETURN <string>id AS id;";
        let mut db_response = self
            .db
            .query(sql)
            .bind(("arg_source", self.agent_source.clone()))
            .bind(("instance", self.agent_instance.clone()))
            .bind(("prompt", prompt.to_string()))
            .bind(("response", response.response.clone()))
            .bind(("arg_tool", self.tool_name.clone()))
            .bind(("arg_session", response.session_id.clone()))
            .bind(("metadata", metadata))
            .await
            .map_err(|e| AgentError::CliError(format!("db insert failed: {}", e)))?;

        eprintln!(
            "[DEBUG persisted.rs] Raw SurrealDB response: {:?}",
            db_response
        );

        let created: Vec<IdResult> = db_response
            .take::<Vec<IdResult>>(0)
            .map_err(|e| AgentError::CliError(format!("db response failed: {}", e)))?;

        let exchange_id = created.first().map(|row| row.id.clone()).ok_or_else(|| {
            AgentError::ParseError(format!("missing exchange id; created={:?}", created))
        })?;

        upsert_tool_session(
            self.db.as_ref(),
            self.tool_name.clone(),
            response.session_id.clone(),
            exchange_id.to_string(),
        )
        .await
        .map_err(|e| AgentError::CliError(format!("db tool session upsert failed: {}", e)))?;

        response.exchange_id = Some(exchange_id.clone());
        Ok(response)
    }
}
