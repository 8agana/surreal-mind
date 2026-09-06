pub mod clients;
pub mod cognitive;
pub mod config;
pub mod deserializers;
pub mod embeddings;
pub mod error;
pub mod indexes;
pub mod maintenance;
pub mod schemas;
pub mod serializers;
pub mod server;
pub mod tools;
pub mod utils;
pub mod version;

// Re-export maintenance types and functions for backwards compatibility
pub use maintenance::{
    KgEmbedStats, ReembedKgStats, ReembedStats, run_kg_embed, run_reembed, run_reembed_kg,
};

// Load env from a simple, standardized location resolution.
// Routes through config::load_env_file() (fed-93bfee #216) so this and every
// other dotenv-loading call site reachable by the db_integration test suite
// share exactly one resolution rule (honors SURR_ENV_FILE when set, with no
// fallback to an ancestor .env; falls back to ordinary dotenvy::dotenv()
// discovery only when SURR_ENV_FILE is unset).
pub fn load_env() {
    config::load_env_file();
}
