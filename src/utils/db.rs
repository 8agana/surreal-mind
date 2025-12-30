//! Database connection utilities

use crate::error::Result;
use reqwest::Client;
use std::time::Duration;
use surrealdb::engine::remote::ws::Client as WsClient;
use surrealdb::Surreal;

/// Configuration for HTTP SQL client
pub struct HttpSqlConfig {
    pub base_url: String,
    pub namespace: String,
    pub database: String,
    pub username: String,
    pub password: String,
    pub component: String,
    pub timeout_secs: u64,
}

impl HttpSqlConfig {
    /// Create from a Config object
    pub fn from_config(config: &crate::config::Config, component: &str) -> Self {
        let host = config.system.database_url.clone();
        let base_url = if host.starts_with("http://") || host.starts_with("https://") {
            host
        } else if host.starts_with("ws://") {
            format!(
                "http://{}",
                host.trim_start_matches("ws://").trim_end_matches('/')
            )
        } else if host.starts_with("wss://") {
            format!(
                "https://{}",
                host.trim_start_matches("wss://").trim_end_matches('/')
            )
        } else {
            format!("http://{}", host.trim_end_matches('/'))
        };

        Self {
            base_url,
            namespace: config.system.database_ns.clone(),
            database: config.system.database_db.clone(),
            username: config.runtime.database_user.clone(),
            password: config.runtime.database_pass.clone(),
            component: component.to_string(),
            timeout_secs: 20,
        }
    }

    /// Build the SQL endpoint URL
    pub fn sql_url(&self) -> String {
        format!("{}/sql", self.base_url.trim_end_matches('/'))
    }

    /// Build the User-Agent string
    pub fn user_agent(&self) -> String {
        let mut ua = format!(
            "surreal-mind/{} (component={}; ns={}; db={})",
            env!("CARGO_PKG_VERSION"),
            self.component,
            self.namespace,
            self.database
        );
        if let Ok(commit) = std::env::var("SURR_COMMIT_HASH") {
            ua.push_str(&format!("; commit={}", &commit[..7.min(commit.len())]));
        }
        ua
    }

    /// Create an HTTP client configured for SQL queries
    pub fn build_client(&self) -> reqwest::Result<Client> {
        Client::builder()
            .timeout(Duration::from_secs(self.timeout_secs))
            .user_agent(self.user_agent())
            .build()
    }
}

/// Upsert a tool session row in a single transaction for continuity tracking.
pub async fn upsert_tool_session(
    db: &Surreal<WsClient>,
    tool: String,
    session: String,
    exchange: String,
) -> Result<()> {
    let exchange_id = normalize_exchange_id(exchange);
    let sql = r#"
        LET $updated = (
            UPDATE tool_sessions
            SET exchange_count += 1,
                last_agent_session_id = $arg_session,
                last_exchange_id = type::thing($exchange_id),
                last_updated = time::now()
            WHERE tool_name = $arg_tool
        );
        IF count($updated) = 0 THEN
            CREATE tool_sessions CONTENT {
                tool_name: $arg_tool,
                last_agent_session_id: $arg_session,
                last_exchange_id: type::thing($exchange_id),
                exchange_count: 1,
                last_updated: time::now()
            };
        END;
    "#;

    db.query(sql)
        .bind(("arg_tool", tool))
        .bind(("arg_session", session))
        .bind(("exchange_id", exchange_id))
        .await?;

    Ok(())
}

fn normalize_exchange_id(exchange: String) -> String {
    if exchange.contains(':') {
        exchange
    } else {
        format!("agent_exchanges:{}", exchange)
    }
}
