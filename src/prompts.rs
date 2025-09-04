//! Self-aware prompt registry for consciousness persistence
//! 
//! This module manages the system's understanding of its own cognitive patterns through
//! versioned, traceable prompts. Each prompt has a lineage, purpose, and explicit constraints,
//! enabling the system to reason about and improve its own thought processes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// Policy constraints that a prompt must respect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptConstraints {
    /// Whether the prompt must respect MCP_NO_LOG
    pub respects_mcp_no_log: bool,
    /// Whether mixed embedding dimensions are allowed
    pub allows_mixed_dims: bool,
    /// Whether KG-only injection is enforced
    pub kg_only_injection: bool,
    /// Additional guardrails as string list
    pub additional_guardrails: Vec<String>,
}

impl Default for PromptConstraints {
    fn default() -> Self {
        Self {
            respects_mcp_no_log: true,
            allows_mixed_dims: false,
            kg_only_injection: true,
            additional_guardrails: vec![],
        }
    }
}

/// Represents a prompt's evolution history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptLineage {
    /// Parent prompt ID if this is a refinement
    pub parent_id: Option<String>,
    /// Git-style SHA1 checksum of the prompt content
    pub checksum: String,
    /// When this version was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Who/what created this version
    pub created_by: String,
    /// Rationale for this version
    pub change_rationale: Option<String>,
}

/// Core prompt definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    /// Stable identifier (format: category-name-v1)
    pub id: String,
    /// Short one-liner description
    pub one_liner: String,
    /// Detailed purpose and intended usage
    pub purpose: String,
    /// Input parameters with types and descriptions
    pub inputs: HashMap<String, String>,
    /// Required runtime constraints
    pub constraints: PromptConstraints,
    /// Version tracking
    pub version: String,
    /// Evolution history
    pub lineage: PromptLineage,
    /// Template text
    pub template: String,
}

impl Prompt {
    /// Create a new prompt with basic validation
    pub fn new(
        id: impl Into<String>,
        one_liner: impl Into<String>,
        purpose: impl Into<String>,
        template: impl Into<String>,
        inputs: HashMap<String, String>,
        constraints: Option<PromptConstraints>,
        parent_id: Option<String>,
        change_rationale: Option<String>,
    ) -> Self {
        let template = template.into();
        let checksum = sha1_checksum(&template);
        
        Self {
            id: id.into(),
            one_liner: one_liner.into(),
            purpose: purpose.into(),
            inputs,
            constraints: constraints.unwrap_or_default(),
            version: "1.0.0".to_string(), // Semantic versioning
            lineage: PromptLineage {
                parent_id,
                checksum,
                created_at: chrono::Utc::now(),
                created_by: "surreal-mind".to_string(),
                change_rationale,
            },
            template,
        }
    }
}

/// Generate a SHA1 checksum of prompt content
fn sha1_checksum(content: &str) -> String {
    use sha1::{Sha1, Digest};
    let mut hasher = Sha1::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

/// Registry of all known prompts with their metadata
#[derive(Debug, Default)]
pub struct PromptRegistry {
    prompts: HashMap<String, Arc<Prompt>>,
}

impl PromptRegistry {
    /// Create new registry with core prompts
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry.register_core_prompts();
        registry
    }
    
    /// Add a prompt to the registry
    pub fn register(&mut self, prompt: Prompt) {
        self.prompts.insert(prompt.id.clone(), Arc::new(prompt));
    }
    
    /// Get a prompt by ID
    pub fn get(&self, id: &str) -> Option<Arc<Prompt>> {
        self.prompts.get(id).cloned()
    }
    
    /// List all prompts
    pub fn list(&self) -> Vec<Arc<Prompt>> {
        self.prompts.values().cloned().collect()
    }
    
    /// Register the core set of prompts
    fn register_core_prompts(&mut self) {
        // Technical thinking patterns
        let tech_inputs: HashMap<String, String> = [
            ("content".into(), "The technical concept or issue to analyze".into()),
            ("injection_scale".into(), "Memory retrieval distance (0-5)".into()),
            ("significance".into(), "Importance weight (0.0-1.0)".into()),
        ].into();

        // Think Plan
        self.register(Prompt::new(
            "think-plan-v1",
            "Architecture and strategy thinking with high context",
            "Structural analysis of technical challenges using systems thinking and first principles",
            "Given the technical challenge:\n{{content}}\n\nAnalyze using:\n- Systems decomposition\n- Component relationships\n- Implementation strategy\n- Risk assessment\n\nConsider existing patterns from injection_scale={{injection_scale}} with significance={{significance}}",
            tech_inputs.clone(),
            Some(PromptConstraints {
                respects_mcp_no_log: true,
                allows_mixed_dims: false,
                kg_only_injection: true,
                additional_guardrails: vec![
                    "No raw thought injection".into(),
                    "Respect existing architecture".into(),
                ],
            }),
            None,
            None,
        ));

        // Think Debug
        self.register(Prompt::new(
            "think-debug-v1", 
            "Root cause analysis with maximum context",
            "Systematic debugging using error traces, logs, and relevant memory context",
            "Debug the issue:\n{{content}}\n\nAnalysis steps:\n1. Reproduce and isolate\n2. Trace error path\n3. Examine state\n4. Test hypotheses\n5. Verify fix\n\nLeverage context from injection_scale={{injection_scale}} with significance={{significance}}",
            tech_inputs,
            Some(PromptConstraints {
                respects_mcp_no_log: true,
                allows_mixed_dims: false,
                kg_only_injection: true,
                additional_guardrails: vec![
                    "Verify all assumptions".into(),
                    "Document root cause".into(),
                ],
            }),
            None,
            None,
        ));
        
        // Memory Search (example of prompt evolution)
        self.register(Prompt::new(
            "think-search-v2",
            "Semantic search with graph expansion",
            "Search over thoughts and knowledge graph with configurable expansion",
            "Search query: {{content}}\nExpand graph: {{expand_graph}}\nDepth: {{graph_depth}}\n\nExecute semantic search with:\n- Dimension validation\n- Cosine similarity\n- Graph traversal when enabled\n- Proper result ranking",
            [
                ("content".into(), "Search query text".into()),
                ("expand_graph".into(), "Whether to expand via graph".into()),
                ("graph_depth".into(), "How far to traverse (0-2)".into()),
            ].into(),
            Some(PromptConstraints {
                respects_mcp_no_log: true,
                allows_mixed_dims: false,
                kg_only_injection: true,
                additional_guardrails: vec![
                    "Validate dimensions before cosine".into(),
                    "Respect graph depth limits".into(),
                ],
            }),
            Some("think-search-v1".into()),
            Some("Added graph expansion with safety limits".into()),
        ));
    }
}

// Database schema for prompt invocations
pub const PROMPT_INVOCATION_SCHEMA: &str = r#"
-- Track prompt usage and outcomes
DEFINE TABLE prompt_invocations SCHEMAFULL;
DEFINE FIELD prompt_id ON prompt_invocations TYPE string;
DEFINE FIELD version ON prompt_invocations TYPE string;
DEFINE FIELD tool ON prompt_invocations TYPE string;
DEFINE FIELD created_at ON prompt_invocations TYPE datetime;
DEFINE FIELD latency_ms ON prompt_invocations TYPE number;
DEFINE FIELD tokens_in ON prompt_invocations TYPE number;
DEFINE FIELD tokens_out ON prompt_invocations TYPE number;
DEFINE FIELD outcome ON prompt_invocations TYPE string;  -- success, error, refusal
DEFINE FIELD error_type ON prompt_invocations TYPE string OPTIONAL;
DEFINE FIELD coverage_score ON prompt_invocations TYPE float OPTIONAL;  -- 0.0-1.0
DEFINE FIELD notes ON prompt_invocations TYPE string OPTIONAL;

-- Index for efficient lookups
DEFINE INDEX prompt_invocations_by_id ON prompt_invocations FIELDS prompt_id;
DEFINE INDEX prompt_invocations_by_date ON prompt_invocations FIELDS created_at;
"#;
