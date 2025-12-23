use crate::error::{Result, SurrealMindError};
use crate::gemini::GeminiClient;
use crate::schemas::{memories_populate_output_schema, memories_populate_schema};
use crate::server::SurrealMindServer;
use crate::sessions::{clear_tool_session, get_tool_session, store_tool_session};
use chrono::Utc;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;
use surrealdb::engine::any::Any;
use uuid::Uuid;

/// Parameters for the memories_populate tool
#[derive(Debug, Deserialize, Serialize)]
pub struct MemoriesPopulateRequest {
    /// Source of thoughts to process
    #[serde(default = "default_source")]
    pub source: String, // "unprocessed" | "chain_id" | "date_range"

    /// Filter by chain_id (if source = "chain_id")
    pub chain_id: Option<String>,

    /// Filter by date (if source = "date_range")
    pub since: Option<String>,
    pub until: Option<String>,

    /// Maximum thoughts to process per call
    #[serde(default = "default_limit")]
    pub limit: u32,

    /// Auto-approve high-confidence extractions
    #[serde(default)]
    pub auto_approve: bool,

    /// Confidence threshold for auto-approval (0.0 - 1.0)
    #[serde(default = "default_threshold")]
    pub confidence_threshold: f32,

    /// Run challenge pass after extraction
    #[serde(default)]
    pub challenge: bool,

    /// Inherit session from another tool
    pub inherit_session_from: Option<String>,
}

fn default_source() -> String {
    "unprocessed".to_string()
}
fn default_limit() -> u32 {
    20
}
fn default_threshold() -> f32 {
    0.8
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoriesPopulateResponse {
    pub thoughts_processed: u32,
    pub entities_extracted: u32,
    pub relationships_extracted: u32,
    pub observations_extracted: u32,
    pub boundaries_extracted: u32,
    pub staged_for_review: u32,
    pub auto_approved: u32,
    pub extraction_batch_id: String,
    pub gemini_session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedMemory {
    pub kind: String, // "entity" | "relationship" | "observation" | "boundary"
    pub data: serde_json::Value,
    pub confidence: f32,
    pub source_thought_ids: Vec<String>,
    pub extraction_batch_id: String,
    pub extracted_at: String,
    pub extraction_prompt_version: String,
}

// Extraction prompt stored inline
const EXTRACTION_PROMPT_VERSION: &str = "extraction_v1";
const EXTRACTION_PROMPT: &str = r#"
You are extracting knowledge graph entries from a collection of thoughts.

For each thought, identify:

1. **Entities** - People, projects, concepts, tools, systems
   - Format: { "name": "...", "type": "person|project|concept|tool|system", "description": "..." }

2. **Relationships** - How entities connect
   - Format: { "from": "entity_name", "to": "entity_name", "relation": "...", "description": "..." }

3. **Observations** - Insights, patterns, lessons learned
   - Format: { "content": "...", "context": "...", "tags": [...] }

4. **Boundaries** - Things explicitly rejected or avoided (who_i_choose_not_to_be)
   - Format: { "rejected": "...", "reason": "...", "context": "..." }

For each extraction, provide a confidence score (0.0-1.0).

Return JSON:
{
  "entities": [...],
  "relationships": [...],
  "observations": [...],
  "boundaries": [...],
  "summary": "Brief summary of what was extracted"
}

THOUGHTS TO PROCESS:
---
{thoughts}
---
"#;
