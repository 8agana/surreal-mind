//! Maintenance operations module.
//!
//! This module contains maintenance and administrative functions for the surreal-mind system,
//! including re-embedding operations for thoughts and knowledge graph entities.

pub mod reembed;

// Re-export public items for backwards compatibility
pub use reembed::{
    KgEmbedStats, ReembedKgStats, ReembedStats, run_kg_embed, run_reembed, run_reembed_kg,
};
