//! Database connection utilities

use reqwest::Client;
use std::time::Duration;

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
        let base_url = if host.starts_with("http") {
            host
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