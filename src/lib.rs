pub mod clients;
pub mod cognitive;
pub mod config;
pub mod deserializers;
pub mod embeddings;
pub mod error;
pub mod indexes;
pub mod maintenance;
pub mod registry;
pub mod schemas;
pub mod serializers;
pub mod server;
pub mod tools;
pub mod utils;
pub mod workspace;

// Re-export maintenance types and functions for backwards compatibility
pub use maintenance::{
    KgEmbedStats, ReembedKgStats, ReembedStats, run_kg_embed, run_reembed, run_reembed_kg,
};

// Load env from a simple, standardized location resolution.
// This uses dotenvy::dotenv().ok() which loads .env if present and silently ignores if missing.
pub fn load_env() {
    let _ = dotenvy::dotenv();
}
