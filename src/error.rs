//! Domain-specific error types for surreal-mind

use thiserror::Error;

/// Main error type for the surreal-mind MCP server
#[derive(Error, Debug)]
pub enum SurrealMindError {
    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("Database error: {message}")]
    Database { message: String },

    #[error("Embedding provider error: {message}")]
    Embedding { message: String },

    #[error("MCP protocol error: {message}")]
    Mcp { message: String },

    #[error("Cognitive framework error: {message}")]
    Cognitive { message: String },

    #[error("Knowledge graph error: {message}")]
    KnowledgeGraph { message: String },

    #[error("Serialization error: {message}")]
    Serialization { message: String },

    #[error("Timeout error: {operation} timed out after {timeout_ms}ms")]
    Timeout { operation: String, timeout_ms: u64 },

    #[error("Validation error: {message}")]
    Validation { message: String },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl From<anyhow::Error> for SurrealMindError {
    fn from(err: anyhow::Error) -> Self {
        SurrealMindError::Internal {
            message: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for SurrealMindError {
    fn from(err: serde_json::Error) -> Self {
        SurrealMindError::Serialization {
            message: err.to_string(),
        }
    }
}

impl From<surrealdb::Error> for SurrealMindError {
    fn from(err: surrealdb::Error) -> Self {
        SurrealMindError::Database {
            message: err.to_string(),
        }
    }
}

impl From<rmcp::ErrorData> for SurrealMindError {
    fn from(err: rmcp::ErrorData) -> Self {
        SurrealMindError::Mcp {
            message: err.message.to_string(),
        }
    }
}

impl From<reqwest::Error> for SurrealMindError {
    fn from(err: reqwest::Error) -> Self {
        SurrealMindError::Embedding {
            message: format!("HTTP request failed: {}", err),
        }
    }
}

impl From<chrono::ParseError> for SurrealMindError {
    fn from(err: chrono::ParseError) -> Self {
        SurrealMindError::Validation {
            message: format!("Date parsing error: {}", err),
        }
    }
}

/// Convert SurrealMindError to MCP error
impl From<SurrealMindError> for rmcp::ErrorData {
    fn from(err: SurrealMindError) -> Self {
        let (code, message) = match err {
            SurrealMindError::Config { .. } => (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                format!("Configuration error: {}", err),
            ),
            SurrealMindError::Database { .. } => (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                format!("Database error: {}", err),
            ),
            SurrealMindError::Embedding { .. } => (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                format!("Embedding error: {}", err),
            ),
            SurrealMindError::Mcp { .. } => (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                format!("MCP protocol error: {}", err),
            ),
            SurrealMindError::Cognitive { .. } => (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                format!("Cognitive framework error: {}", err),
            ),
            SurrealMindError::KnowledgeGraph { .. } => (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                format!("Knowledge graph error: {}", err),
            ),
            SurrealMindError::Serialization { .. } => (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                format!("Serialization error: {}", err),
            ),
            SurrealMindError::Timeout { .. } => (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                format!("Operation timeout: {}", err),
            ),
            SurrealMindError::Validation { .. } => (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                format!("Validation error: {}", err),
            ),
            SurrealMindError::Internal { .. } => (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                format!("Internal error: {}", err),
            ),
        };

        rmcp::ErrorData {
            code,
            message: message.into(),
            data: None,
        }
    }
}

/// Result type alias for SurrealMind operations
pub type Result<T> = std::result::Result<T, SurrealMindError>;
