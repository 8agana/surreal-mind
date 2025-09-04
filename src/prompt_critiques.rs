//! Prompt critique storage and analysis
//! 
//! This module enables the system to store and analyze feedback about its own prompts,
//! treating critiques as first-class thoughts that can inform prompt evolution.

use crate::error::Result;
use crate::prompts::Prompt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// A critique of a prompt's effectiveness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptCritique {
    pub prompt_id: String,
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub critique_type: CritiqueType,
    pub content: String,
    pub suggested_changes: Option<String>,
    pub impact_areas: Vec<String>,
    pub priority: CritiquePriority,
    pub status: CritiqueStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CritiqueType {
    Improvement,
    Bug,
    Clarification,
    Extension,
    Constraint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CritiquePriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CritiqueStatus {
    Open,
    UnderReview,
    Accepted,
    Rejected,
    Implemented,
}

impl std::fmt::Display for CritiqueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Improvement => write!(f, "improvement"),
            Self::Bug => write!(f, "bug"),
            Self::Clarification => write!(f, "clarification"),
            Self::Extension => write!(f, "extension"),
            Self::Constraint => write!(f, "constraint"),
        }
    }
}

impl PromptCritique {
    /// Create a new critique
    pub fn new(
        prompt: &Prompt,
        critique_type: CritiqueType,
        content: impl Into<String>,
        suggested_changes: Option<String>,
        impact_areas: Vec<String>,
        priority: CritiquePriority,
    ) -> Self {
        Self {
            prompt_id: prompt.id.clone(),
            version: prompt.version.clone(),
            created_at: Utc::now(),
            critique_type,
            content: content.into(),
            suggested_changes,
            impact_areas,
            priority,
            status: CritiqueStatus::Open,
        }
    }

    /// Store the critique as a thought with proper linkage
    pub async fn store_as_thought(&self, server: &crate::server::SurrealMindServer) -> Result<String> {
        // Create a thought with critique metadata
        let thought_content = format!(
            "Prompt Critique: {}\n\nType: {}\nPriority: {:?}\nStatus: {:?}\n\nContent:\n{}\n\n{}",
            self.prompt_id,
            self.critique_type,
            self.priority,
            self.status,
            self.content,
            self.suggested_changes
                .as_ref()
                .map(|s| format!("Suggested Changes:\n{}", s))
                .unwrap_or_default()
        );

        // Store as a thought with special tags and metadata
        let created: Vec<serde_json::Value> = server
            .db
            .query(
                "CREATE thoughts SET
                    content = $content,
                    created_at = time::now(),
                    tags = ['prompt_critique', type::string($critique_type)],
                    critique_data = $critique,
                    prompt_ref = type::thing('prompts', $prompt_id)
                RETURN meta::id(id)",
            )
.bind(("content", thought_content))
            .bind(("critique_type", self.critique_type.to_string()))
            .bind(("critique", json!({
                "prompt_id": self.prompt_id,
                "version": self.version,
                "type": self.critique_type.to_string(),
                "priority": self.priority,
                "status": self.status,
                "impact_areas": self.impact_areas
            })))
.bind(("prompt_id", self.prompt_id.clone()))
            .await?
            .take(0)?;

        // Extract and return the thought ID
        let thought_id = created
            .first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::SurrealMindError::Internal {
                message: "Failed to get ID of created critique thought".into(),
            })?
            .to_string();

        Ok(thought_id)
    }
}

impl crate::server::SurrealMindServer {
    /// Get all critiques for a prompt
    pub async fn get_prompt_critiques(&self, prompt_id: &str) -> Result<Vec<PromptCritique>> {
        let critiques: Vec<serde_json::Value> = self
            .db
            .query(
                "SELECT 
                    meta::id(id) as id,
                    content,
                    critique_data.* as critique,
                    created_at
                FROM thoughts
                WHERE critique_data.prompt_id = $id
                ORDER BY created_at DESC",
            )
            .bind(("id", prompt_id.to_string()))
            .await?
            .take(0)?;

        let mut results = Vec::new();
        for c in critiques {
            if let Some(data) = c.get("critique") {
                // Parse critique data from the stored JSON
                let critique: PromptCritique = serde_json::from_value(data.clone())?;
                results.push(critique);
            }
        }

        Ok(results)
    }

    /// Get prompt evolution suggestions based on critiques
    pub async fn get_prompt_evolution_suggestions(&self, prompt_id: &str) -> Result<Vec<String>> {
        let critiques = self.get_prompt_critiques(prompt_id).await?;
        
        // Group by type and filter for actionable items
        let mut improvements = Vec::new();
        let mut bugs = Vec::new();
        let mut clarifications = Vec::new();
        
        for c in critiques {
            if c.status == CritiqueStatus::Open || c.status == CritiqueStatus::UnderReview {
                match c.critique_type {
                    CritiqueType::Improvement => improvements.push(c),
                    CritiqueType::Bug => bugs.push(c),
                    CritiqueType::Clarification => clarifications.push(c),
                    _ => {}
                }
            }
        }
        
        let mut suggestions = Vec::new();
        
        // Critical bugs first
        for bug in bugs.iter().filter(|c| matches!(c.priority, CritiquePriority::Critical)) {
            suggestions.push(format!("🔴 Critical Bug: {}", bug.content));
            if let Some(fix) = &bug.suggested_changes {
                suggestions.push(format!("   Fix: {}", fix));
            }
        }
        
        // High priority improvements
        for imp in improvements.iter().filter(|c| matches!(c.priority, CritiquePriority::High)) {
            suggestions.push(format!("⭐ High Priority: {}", imp.content));
            if let Some(changes) = &imp.suggested_changes {
                suggestions.push(format!("   Changes: {}", changes));
            }
        }
        
        // Needed clarifications
        for clar in clarifications.iter().filter(|c| matches!(c.priority, CritiquePriority::High)) {
            suggestions.push(format!("📝 Needs Clarification: {}", clar.content));
        }
        
        Ok(suggestions)
    }
}

/// Create prompt critique schema
pub const PROMPT_CRITIQUE_SCHEMA: &str = r#"
-- Add critique metadata to thoughts table
DEFINE FIELD critique_data ON thoughts TYPE object {
    prompt_id: string,
    version: string,
    type: string,
    priority: string,
    status: string,
    impact_areas: array
} OPTIONAL;

DEFINE FIELD prompt_ref ON thoughts TYPE record<prompts> OPTIONAL;

-- Index for finding critiques
DEFINE INDEX thoughts_critiques ON thoughts FIELDS critique_data.prompt_id;
"#;
