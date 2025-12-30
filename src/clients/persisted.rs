use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Map, Value};
use surrealdb::engine::remote::ws::Client as WsClient;
use surrealdb::Surreal;
use surrealdb::sql::Number as SurrealNumber;
use surrealdb::sql::Value as SqlValue;
use surrealdb::Value as SurrealValue;

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

        let sql = "CREATE agent_exchanges SET created_at = time::now(), agent_source = $arg_source, agent_instance = $instance, prompt = $prompt, response = $response, tool_name = $arg_tool, session_id = $arg_session, metadata = $metadata RETURN id;";
        let created: SurrealValue = self
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
            .map_err(|e| AgentError::CliError(format!("db insert failed: {}", e)))?
            .take::<surrealdb::Value>(0)
            .map_err(|e| AgentError::CliError(format!("db response failed: {}", e)))?;

        let created = to_json_value(created)
            .map_err(|e| AgentError::CliError(format!("db response serialize failed: {}", e)))?;

        let exchange_id = first_row(&created)
            .and_then(parse_record_id)
            .ok_or_else(|| {
                let created_json =
                    serde_json::to_string(&created).unwrap_or_else(|_| "<unserializable>".to_string());
                AgentError::ParseError(format!("missing exchange id; created={}", created_json))
            })?;

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

fn parse_record_id(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }

    let obj = value.as_object()?;
    if let Some(record) = obj.get("$record").and_then(|v| v.as_str()) {
        return Some(record.to_string());
    }
    if let Some(thing) = obj.get("$thing") {
        return parse_record_id(thing);
    }
    if let (Some(tb), Some(id_val)) = (obj.get("tb"), obj.get("id")) {
        let table = tb.as_str()?;
        let id = parse_record_id_value(id_val).or_else(|| parse_record_id(id_val))?;
        return Some(format!("{}:{}", table, id));
    }

    obj.get("id").and_then(parse_record_id)
}

fn to_json_value(value: SurrealValue) -> Result<Value, serde_json::Error> {
    Ok(to_json_value_inner(value.into()))
}

fn to_json_value_inner(value: SqlValue) -> Value {
    match value {
        SqlValue::None | SqlValue::Null => Value::Null,
        SqlValue::Bool(value) => Value::Bool(value),
        SqlValue::Number(number) => number_to_json(number),
        SqlValue::Strand(value) => Value::String(value.to_string()),
        SqlValue::Duration(value) => Value::String(value.to_string()),
        SqlValue::Datetime(value) => Value::String(value.to_string()),
        SqlValue::Uuid(value) => Value::String(value.to_string()),
        SqlValue::Array(values) => {
            Value::Array(values.into_iter().map(to_json_value_inner).collect())
        }
        SqlValue::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, to_json_value_inner(value)))
                .collect(),
        ),
        SqlValue::Geometry(value) => Value::String(value.to_string()),
        SqlValue::Bytes(value) => Value::String(value.to_string()),
        SqlValue::Thing(value) => Value::String(value.to_string()),
        other => Value::String(other.to_string()),
    }
}

fn number_to_json(number: SurrealNumber) -> Value {
    match number {
        SurrealNumber::Int(value) => Value::Number(serde_json::Number::from(value)),
        SurrealNumber::Float(value) => serde_json::Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(value.to_string())),
        SurrealNumber::Decimal(value) => Value::String(value.to_string()),
        _ => Value::String(number.to_string()),
    }
}

fn first_row(value: &Value) -> Option<&Value> {
    match value {
        Value::Array(values) => values.first(),
        Value::Null => None,
        other => Some(other),
    }
}

fn parse_record_id_value(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    if let Some(n) = value.as_i64() {
        return Some(n.to_string());
    }
    if let Some(n) = value.as_u64() {
        return Some(n.to_string());
    }
    if let Some(uuid) = value.get("$uuid").and_then(|v| v.as_str()) {
        return Some(uuid.to_string());
    }
    if let Some(ulid) = value.get("$ulid").and_then(|v| v.as_str()) {
        return Some(ulid.to_string());
    }
    None
}
