//! Shared types and constants for the thinking module
//!
//! This module contains type definitions used across multiple thinking submodules.
//! Extracted to reduce cognitive load when agents work on specific thinking components.

use serde::{Deserialize, Serialize};

/// Maximum content size in bytes (100KB)
pub const MAX_CONTENT_SIZE: usize = 100 * 1024;

/// Default contradiction patterns for hypothesis verification (case-insensitive)
pub const CONTRADICTION_PATTERNS: &[&str] = &[
    "not",
    "no",
    "cannot",
    "false",
    "incorrect",
    "fails",
    "broken",
    "doesn't",
    "isn't",
    "won't",
];

/// Modes for legacymind_think routing
#[derive(Debug, Clone, PartialEq)]
pub enum ThinkMode {
    Debug,
    Build,
    Plan,
    Stuck,
    Question,
    Conclude,
}

/// Evidence item for hypothesis verification
#[derive(Debug, Clone, Serialize)]
pub struct EvidenceItem {
    pub table: String,
    pub id: String,
    pub text: String,
    pub similarity: f32,
    pub provenance: Option<serde_json::Value>,
}

/// Verification result for hypothesis verification
#[derive(Debug, Clone, Serialize)]
pub struct VerificationResult {
    pub hypothesis: String,
    pub supporting: Vec<EvidenceItem>,
    pub contradicting: Vec<EvidenceItem>,
    pub confidence_score: f32,
    pub suggested_revision: Option<String>,
    pub telemetry: serde_json::Value,
}

/// Parameters for legacymind_think
#[derive(Debug, Deserialize)]
pub struct LegacymindThinkParams {
    pub content: String,
    #[serde(default)]
    pub hint: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u8_forgiving"
    )]
    pub injection_scale: Option<u8>,
    #[serde(default, deserialize_with = "crate::deserializers::de_option_tags")]
    pub tags: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_f32_forgiving"
    )]
    pub significance: Option<f32>,
    #[serde(default)]
    pub verbose_analysis: Option<bool>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub chain_id: Option<String>,
    #[serde(default)]
    pub previous_thought_id: Option<String>,
    #[serde(default)]
    pub revises_thought: Option<String>,
    #[serde(default)]
    pub branch_from: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_f32_forgiving"
    )]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub hypothesis: Option<String>,
    #[serde(default)]
    pub needs_verification: Option<bool>,
    #[serde(default)]
    pub verify_top_k: Option<usize>,
    #[serde(default)]
    pub min_similarity: Option<f32>,
    #[serde(default)]
    pub evidence_limit: Option<usize>,
    #[serde(default)]
    pub contradiction_patterns: Option<Vec<String>>,
}

/// Result struct for continuity links resolution
#[derive(Debug, Serialize)]
pub struct ContinuityResult {
    pub session_id: Option<String>,
    pub chain_id: Option<String>,
    pub previous_thought_id: Option<String>,
    pub revises_thought: Option<String>,
    pub branch_from: Option<String>,
    pub confidence: Option<f32>,
    pub links_resolved: serde_json::Value,
}

/// Process a database query result for continuity link resolution
/// Takes the original ID and the query result, returns (resolved_id, resolution_type)
/// When the query result is empty, preserves the ID as a string for future resolution
pub fn process_continuity_query_result(
    original_id: String,
    query_result: Vec<serde_json::Value>,
) -> (Option<String>, &'static str) {
    // Normalize the ID format
    let normalized_id = if original_id.starts_with("thoughts:") {
        original_id
    } else {
        format!("thoughts:{}", original_id)
    };

    // Check if the record exists based on query result
    if !query_result.is_empty() {
        // Record found in database
        (Some(normalized_id), "record")
    } else {
        // Record not found - preserve as string for future resolution
        tracing::warn!(
            "Continuity link {} not found in database, preserving as string for future resolution",
            normalized_id
        );
        (Some(normalized_id), "string")
    }
}
