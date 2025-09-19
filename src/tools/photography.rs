//! Photography-scoped tools that operate on an isolated SurrealDB namespace/database

use super::inner_voice::{InnerVoiceContext, InnerVoiceRetrieveParams, run_inner_voice};
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};

impl SurrealMindServer {
    /// Handle photography_think by reusing think_convo semantics against the photography DB
    pub async fn handle_photography_think(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let dbp = if let Some(dbp) = &self.db_photo {
            dbp.clone()
        } else {
            self.connect_photo_db().await?
        };
        let photo = self.clone_with_db(dbp);
        // Reuse conversational defaults (origin="human", scale=1, significance=0.5)
        photo.handle_convo_think(request).await
    }

    /// Multiplexed photography_memories tool; dispatches to create/search/moderate on the photography DB
    pub async fn handle_photography_memories(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request
            .arguments
            .clone()
            .ok_or_else(|| SurrealMindError::Mcp {
                message: "Missing parameters".into(),
            })?;
        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("search");

        let dbp = if let Some(dbp) = &self.db_photo {
            dbp.clone()
        } else {
            self.connect_photo_db().await?
        };
        let photo = self.clone_with_db(dbp);

        match mode {
            "create" => photo.handle_knowledgegraph_create(request).await,
            "moderate" => photo.handle_knowledgegraph_moderate(request).await,
            _ => photo.handle_knowledgegraph_search(request).await,
        }
    }

    /// photography_thoughts_search: wrapper for think_search against photography DB
    pub async fn handle_photography_thoughts_search(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let dbp = if let Some(dbp) = &self.db_photo {
            dbp.clone()
        } else {
            self.connect_photo_db().await?
        };
        let photo = self.clone_with_db(dbp);
        photo.handle_search_thoughts(request).await
    }

    /// photography_memories_search: wrapper for memories_search against photography DB
    pub async fn handle_photography_memories_search(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let dbp = if let Some(dbp) = &self.db_photo {
            dbp.clone()
        } else {
            self.connect_photo_db().await?
        };
        let photo = self.clone_with_db(dbp);
        photo.handle_knowledgegraph_search(request).await
    }

    /// Unified photography_search: memories by default, optional thoughts
    pub async fn handle_photography_unified_search(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let dbp = if let Some(dbp) = &self.db_photo {
            dbp.clone()
        } else {
            self.connect_photo_db().await?
        };
        let photo = self.clone_with_db(dbp);
        // Reuse the same inner logic as unified_search but against the photography DB by calling the inner helper
        crate::tools::unified_search::unified_search_inner(&photo, request).await
    }

    /// photography_voice: grounded synthesis tool for photography namespace
    pub async fn handle_photography_voice(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let dbp = if let Some(dbp) = &self.db_photo {
            dbp.clone()
        } else {
            self.connect_photo_db().await?
        };
        let photo = self.clone_with_db(dbp);

        // Extract params similar to inner_voice handler
        let args = request
            .arguments
            .clone()
            .ok_or_else(|| SurrealMindError::Mcp {
                message: "Missing parameters".into(),
            })?;
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SurrealMindError::Mcp {
                message: "Missing query".into(),
            })?
            .to_string();

        let params = InnerVoiceRetrieveParams {
            query,
            top_k: Some(args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(10) as usize),
            floor: args.get("floor").and_then(|v| v.as_f64()).map(|f| f as f32),
            mix: Some(args.get("mix").and_then(|v| v.as_f64()).unwrap_or(0.6) as f32),
            include_private: Some(
                args.get("include_private")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            ),
            include_tags: args
                .get("include_tags")
                .and_then(|v| v.as_array())
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect(),
            exclude_tags: args
                .get("exclude_tags")
                .and_then(|v| v.as_array())
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect(),
            auto_extract_to_kg: Some(
                args.get("auto_extract_to_kg")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            ),
            previous_thought_id: args
                .get("previous_thought_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            include_feedback: Some(
                args.get("include_feedback")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
            ),
            feedback_max_lines: Some(
                args.get("feedback_max_lines")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(3) as usize,
            ),
        };

        let ctx = InnerVoiceContext {
            runtime: photo.build_inner_voice_runtime(),
            hooks: Default::default(),
        };

        run_inner_voice(&photo, &params, &ctx).await
    }

    /// photography_moderate: dedicated tool for moderating photography knowledge-graph candidates
    pub async fn handle_photography_moderate(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let dbp = if let Some(dbp) = &self.db_photo {
            dbp.clone()
        } else {
            self.connect_photo_db().await?
        };
        let photo = self.clone_with_db(dbp);
        // Dispatch to the same moderate handler as the main namespace
        photo.handle_knowledgegraph_moderate(request).await
    }
}
