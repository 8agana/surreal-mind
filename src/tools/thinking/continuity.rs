//! Continuity resolution for thought chains
//!
//! This module handles the resolution and validation of continuity links
//! between thoughts: `previous_thought_id`, `revises_thought`, and `branch_from`.
//!
//! Key features:
//! - Database validation of thought references
//! - Self-link prevention (thought can't link to itself)
//! - Deduplication (same ID can't appear in multiple link fields)
//! - Graceful handling of missing thoughts (preserved as string for future resolution)

use super::types::{ContinuityResult, process_continuity_query_result};
use crate::error::Result;
use crate::server::SurrealMindServer;
use std::collections::HashSet;

impl SurrealMindServer {
    /// Resolve continuity links with validation and normalization
    ///
    /// Validates that referenced thought IDs exist in the database, prevents
    /// self-referential links, and deduplicates across link types.
    ///
    /// # Resolution behavior
    /// - If a thought ID exists in DB: stored as normalized "thoughts:id" format
    /// - If a thought ID doesn't exist: preserved as string for future resolution
    /// - Self-links: dropped with "dropped_self_link" status
    /// - Duplicate IDs: later occurrences dropped with "dropped_duplicate" status
    ///
    /// # Arguments
    /// * `new_thought_id` - The ID of the thought being created (to prevent self-links)
    /// * `previous_thought_id` - Optional link to the previous thought in a chain
    /// * `revises_thought` - Optional link to a thought being revised
    /// * `branch_from` - Optional link to a thought this branches from
    #[allow(clippy::single_match, clippy::redundant_pattern_matching)]
    pub(crate) async fn resolve_continuity_links(
        &self,
        new_thought_id: &str,
        previous_thought_id: Option<String>,
        revises_thought: Option<String>,
        branch_from: Option<String>,
    ) -> Result<ContinuityResult> {
        let mut links_resolved = serde_json::Map::new();

        let mut resolved = ContinuityResult {
            session_id: None,
            chain_id: None,
            previous_thought_id: None,
            revises_thought: None,
            branch_from: None,
            confidence: None,
            links_resolved: serde_json::Value::Object(serde_json::Map::new()),
        };

        // Helper function to resolve and validate a thought reference
        let resolve_thought = |id: String| async move {
            // Determine the full ID format for querying
            let full_id = if id.starts_with("thoughts:") {
                id.clone()
            } else {
                format!("thoughts:{}", id)
            };

            // Query the database to check if the record exists
            let check_query = "SELECT id FROM type::thing($id) LIMIT 1";
            let query_result = match self
                .db
                .query(check_query)
                .bind(("id", full_id.clone()))
                .await
            {
                Ok(mut response) => response
                    .take::<Vec<serde_json::Value>>(0)
                    .unwrap_or_default(),
                Err(e) => {
                    tracing::warn!("Failed to query continuity link {}: {}", full_id, e);
                    Vec::new()
                }
            };

            // Process the query result to determine how to handle the ID
            process_continuity_query_result(id, query_result)
        };

        // Resolve each link
        if let Some(id) = previous_thought_id {
            let (resolved_id, resolution_type) = resolve_thought(id).await;
            resolved.previous_thought_id = resolved_id;
            links_resolved.insert(
                "previous_thought_id".to_string(),
                serde_json::Value::String(resolution_type.to_string()),
            );
        }

        if let Some(id) = revises_thought {
            let (resolved_id, resolution_type) = resolve_thought(id).await;
            resolved.revises_thought = resolved_id;
            links_resolved.insert(
                "revises_thought".to_string(),
                serde_json::Value::String(resolution_type.to_string()),
            );
        }

        if let Some(id) = branch_from {
            let (resolved_id, resolution_type) = resolve_thought(id).await;
            resolved.branch_from = resolved_id;
            links_resolved.insert(
                "branch_from".to_string(),
                serde_json::Value::String(resolution_type.to_string()),
            );
        }

        // Prevent self-links
        if resolved
            .previous_thought_id
            .as_ref()
            .map(|id| id.contains(new_thought_id))
            .unwrap_or(false)
        {
            resolved.previous_thought_id = None;
            links_resolved.insert(
                "previous_thought_id".to_string(),
                serde_json::Value::String("dropped_self_link".to_string()),
            );
        }
        if resolved
            .revises_thought
            .as_ref()
            .map(|id| id.contains(new_thought_id))
            .unwrap_or(false)
        {
            resolved.revises_thought = None;
            links_resolved.insert(
                "revises_thought".to_string(),
                serde_json::Value::String("dropped_self_link".to_string()),
            );
        }
        if resolved
            .branch_from
            .as_ref()
            .map(|id| id.contains(new_thought_id))
            .unwrap_or(false)
        {
            resolved.branch_from = None;
            links_resolved.insert(
                "branch_from".to_string(),
                serde_json::Value::String("dropped_self_link".to_string()),
            );
        }

        // Deduplicate (keep first occurrence)
        let mut seen_ids = HashSet::new();
        if let Some(ref id) = resolved.previous_thought_id {
            seen_ids.insert(id.clone());
        }
        if let Some(ref id) = resolved.revises_thought {
            if seen_ids.contains(id) {
                resolved.revises_thought = None;
                links_resolved.insert(
                    "revises_thought".to_string(),
                    serde_json::Value::String("dropped_duplicate".to_string()),
                );
            } else {
                seen_ids.insert(id.clone());
            }
        }
        if let Some(ref id) = resolved.branch_from
            && seen_ids.contains(id)
        {
            resolved.branch_from = None;
            links_resolved.insert(
                "branch_from".to_string(),
                serde_json::Value::String("dropped_duplicate".to_string()),
            );
        }

        resolved.links_resolved = serde_json::Value::Object(links_resolved);
        Ok(resolved)
    }
}
