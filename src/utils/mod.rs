//! Utility functions shared across the codebase

pub mod db;
pub mod math;

// Re-export commonly used utilities
pub use db::HttpSqlConfig;
pub use math::cosine_similarity;