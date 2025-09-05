//! Photography-scoped tools that operate on an isolated SurrealDB namespace/database

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};

impl SurrealMindServer {
    /// Handle photography_think by reusing think_convo semantics against the photography DB
    pub async fn handle_photography_think(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let dbp = if let Some(dbp) = &self.db_photo { dbp.clone() } else { self.connect_photo_db().await? };
        let photo = self.clone_with_db(dbp);
        // Reuse conversational defaults (origin="human", scale=1, significance=0.5)
        photo.handle_convo_think(request).await
    }

    /// Multiplexed photography_memories tool; dispatches to create/search/moderate on the photography DB
    pub async fn handle_photography_memories(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.clone().ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("search");

        let dbp = if let Some(dbp) = &self.db_photo { dbp.clone() } else { self.connect_photo_db().await? };
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
        let dbp = if let Some(dbp) = &self.db_photo { dbp.clone() } else { self.connect_photo_db().await? };
        let photo = self.clone_with_db(dbp);
        photo.handle_search_thoughts(request).await
    }

    /// photography_memories_search: wrapper for memories_search against photography DB
    pub async fn handle_photography_memories_search(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let dbp = if let Some(dbp) = &self.db_photo { dbp.clone() } else { self.connect_photo_db().await? };
        let photo = self.clone_with_db(dbp);
        photo.handle_knowledgegraph_search(request).await
    }

    /// Unified photography_search: memories by default, optional thoughts
    pub async fn handle_photography_unified_search(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let dbp = if let Some(dbp) = &self.db_photo { dbp.clone() } else { self.connect_photo_db().await? };
        let photo = self.clone_with_db(dbp);
        // Reuse the same inner logic as unified_search but against the photography DB by calling the inner helper
        crate::tools::unified_search::unified_search_inner(&photo, request).await
    }
}
