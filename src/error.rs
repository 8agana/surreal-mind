//! Domain-specific error types for surreal-mind

use serde_json::json;
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

    #[error("Feature disabled: {message}")]
    FeatureDisabled { message: String },

    #[error("Embedder unavailable: {message}")]
    EmbedderUnavailable { message: String },

    #[error("Invalid parameters: {message}")]
    InvalidParams { message: String },
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
        let (code, label, details) = match err {
            SurrealMindError::Config { message } => (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "Configuration error",
                message,
            ),
            SurrealMindError::Database { message } => (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "Database error",
                message,
            ),
            SurrealMindError::Embedding { message } => (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "Embedding error",
                message,
            ),
            SurrealMindError::Mcp { message } => (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "MCP protocol error",
                message,
            ),
            SurrealMindError::Cognitive { message } => (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "Cognitive framework error",
                message,
            ),
            SurrealMindError::KnowledgeGraph { message } => (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "Knowledge graph error",
                message,
            ),
            SurrealMindError::Serialization { message } => (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "Serialization error",
                message,
            ),
            SurrealMindError::Timeout {
                operation,
                timeout_ms,
            } => (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "Operation timeout",
                format!("{operation} timed out after {timeout_ms}ms"),
            ),
            SurrealMindError::Validation { message } => (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "Validation error",
                message,
            ),
            SurrealMindError::Internal { message } => (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "Internal error",
                message,
            ),
            SurrealMindError::FeatureDisabled { message } => (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "Feature disabled",
                message,
            ),
            SurrealMindError::EmbedderUnavailable { message } => (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "Embedder unavailable",
                message,
            ),
            SurrealMindError::InvalidParams { message } => (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "Invalid parameters",
                message,
            ),
        };

        rmcp::ErrorData {
            code,
            message: format!("{label}: {details}").into(),
            data: Some(json!({ "details": details })),
        }
    }
}

/// Result type alias for SurrealMind operations
pub type Result<T> = std::result::Result<T, SurrealMindError>;
