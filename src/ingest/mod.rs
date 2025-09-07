//! Ingest module for deterministic extraction of README and CHANGELOG into structured knowledge
//!
//! This module provides the core functionality for parsing documents, generating claims,
//! and creating candidates for the knowledge graph. All extraction is deterministic
//! and rule-based to ensure reproducibility.

use crate::error::Result;
use serde::Serialize;
use std::path::Path;

/// Configuration for document ingestion
#[derive(Debug)]
pub struct IngestConfig {
    pub root_path: String,
    pub project_slug: String,
    pub include_readme: bool,
    pub include_changelog: bool,
    pub claims_only: bool,
    pub verify_claims: bool,
    pub min_similarity: f32,
    pub verify_top_k: usize,
    pub evidence_limit: usize,
    pub batch_size: usize,
    pub continue_on_error: bool,
    pub max_retries: u64,
    pub progress: bool,
    pub prometheus: bool,
    pub json: bool,
}

/// Represented document from filesystem
#[derive(Debug, Serialize)]
pub struct Document {
    pub id: String,
    pub project: String,
    pub path: String,
    pub kind: String, // "readme" or "changelog"
    pub content: String,
    pub hash: String,
    pub commit_sha: String,
}

/// Extracted section from document
#[derive(Debug, Serialize)]
pub struct Section {
    pub id: String,
    pub doc_id: String,
    pub slug: String,
    pub title: String,
    pub level: u8,
    pub content: String,
    pub hash: String,
    pub commit_sha: String,
    pub line_from: usize,
    pub line_to: usize,
}

/// Release information from CHANGELOG
#[derive(Debug, Serialize)]
pub struct Release {
    pub id: String,
    pub semver: String,
    pub date: String,
    pub commit_sha: String,
}

/// CHANGELOG entry
#[derive(Debug, Serialize)]
pub struct ChangelogEntry {
    pub id: String,
    pub release_id: String,
    pub kind: String, // "Added", "Changed", etc.
    pub text: String,
    pub hash: String,
}

/// Deterministic claim extracted from document
#[derive(Debug, Serialize)]
pub struct Claim {
    pub id: String,
    pub source_type: String, // "readme" or "changelog"
    pub source_id: String,
    pub release_id: Option<String>,
    pub commit_sha: String,
    pub claim_text: String,
    pub normalized_text: String,
    pub blake3_hash: String,
    pub embedding: Vec<f32>,
    pub embedding_model: String,
    pub embedding_dim: usize,
    pub created_at: String,
}

/// Candidate for knowledge graph
#[derive(Debug, Serialize)]
pub struct Candidate {
    pub kind: String, // "entity" or "edge"
    pub data: serde_json::Value,
    pub confidence: f32,
    pub provenance: Provenance,
}

/// Provenance information
#[derive(Debug, Serialize)]
pub struct Provenance {
    pub doc_id: String,
    pub section_id: String,
    pub claim_id: String,
    pub commit_sha: String,
    pub line_from: usize,
    pub line_to: usize,
}

/// Result of ingestion process
#[derive(Debug, Serialize)]
pub struct IngestResult {
    pub documents_processed: usize,
    pub sections_extracted: usize,
    pub claims_generated: usize,
    pub candidates_created: usize,
    pub errors: Vec<String>,
}

/// Trait for document parsers
pub trait DocumentParser {
    /// Parse document content and extract sections
    fn parse(content: &str, path: &Path) -> Result<Vec<Section>>;

    /// Extract deterministic claims from sections
    fn extract_claims(sections: &[Section], project_slug: &str) -> Result<Vec<Claim>>;

    /// Generate candidates from claims
    fn generate_candidates(claims: &[Claim], project_slug: &str) -> Result<Vec<Candidate>>;
}

/// Common utility functions
pub mod utils {
    use blake3;
    use chrono::Utc;

    /// Generate stable ID for document
    pub fn generate_doc_id(project: &str, path: &str, kind: &str) -> String {
        format!("{}:{}:{}", project, kind, path.replace("/", "_"))
    }

    /// Generate stable ID for section
    pub fn generate_section_id(doc_id: &str, slug: &str, hash: &str) -> String {
        format!("{}:{}:{}", doc_id, slug, &hash[..8])
    }

    /// Generate stable ID for claim
    pub fn generate_claim_id(source_id: &str, claim_text: &str) -> String {
        let hash = blake3::hash(format!("claim:{}:{}", source_id, claim_text).as_bytes());
        hash.to_hex().to_string()
    }

    /// Generate stable ID for release
    pub fn generate_release_id(semver: &str) -> String {
        format!("v{}", semver)
    }

    /// Generate BLAKE3 hash for claim with prefix
    pub fn hash_claim(text: &str) -> String {
        let hash = blake3::hash(format!("claim:{}", text).as_bytes());
        hash.to_hex().to_string()
    }

    /// Get current timestamp as ISO string
    pub fn current_timestamp() -> String {
        Utc::now().to_rfc3339()
    }
}

/// Metrics for Prometheus export
#[derive(Debug)]
pub struct Metrics {
    pub sections_parsed: usize,
    pub claims_extracted: usize,
    pub candidates_created: usize,
    pub errors_count: usize,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            sections_parsed: 0,
            claims_extracted: 0,
            candidates_created: 0,
            errors_count: 0,
        }
    }

    pub fn as_prometheus(&self) -> String {
        format!(
            "# HELP ingest_sections_parsed_total Total sections parsed\n\
             # TYPE ingest_sections_parsed_total counter\n\
             ingest_sections_parsed_total {}\n\
             # HELP ingest_claims_extracted_total Total claims extracted\n\
             # TYPE ingest_claims_extracted_total counter\n\
             ingest_claims_extracted_total {}\n\
             # HELP ingest_candidates_created_total Total candidates created\n\
             # TYPE ingest_candidates_created_total counter\n\
             ingest_candidates_created_total {}\n\
             # HELP ingest_errors_count_total Total errors during ingestion\n\
             # TYPE ingest_errors_count_total counter\n\
             ingest_errors_count_total {}\n",
            self.sections_parsed, self.claims_extracted, self.candidates_created, self.errors_count
        )
    }
}

pub mod changelog;
pub mod markdown;
