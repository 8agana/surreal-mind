//! Hypothesis verification against the knowledge graph
//!
//! This module provides functionality for verifying hypotheses by finding
//! supporting and contradicting evidence in the knowledge graph.

use crate::error::Result;
use crate::server::SurrealMindServer;
use super::types::{EvidenceItem, VerificationResult, CONTRADICTION_PATTERNS};
use serde_json::json;

impl SurrealMindServer {
    /// Build text from KG entity or observation for embedding
    ///
    /// Constructs a searchable text representation from entity/observation data.
    /// Priority: entity_type suffix > description suffix > name only
    pub(crate) fn build_kg_text(name: &str, data: Option<&serde_json::Value>) -> String {
        let mut text = name.to_string();
        if let Some(d) = data.as_ref().and_then(|v| v.as_object()) {
            if let Some(etype) = d.get("entity_type").and_then(|v| v.as_str()) {
                text = format!("{} ({})", name, etype);
            } else if let Some(desc) = d.get("description").and_then(|v| v.as_str()) {
                text.push_str(" - ");
                text.push_str(desc);
            }
        }
        text
    }

    /// Run hypothesis verification against KG
    ///
    /// Searches the knowledge graph for entities and observations that either
    /// support or contradict the given hypothesis based on semantic similarity.
    ///
    /// # Arguments
    /// * `hypothesis` - The statement to verify
    /// * `top_k` - Maximum candidates to retrieve from each KG table
    /// * `min_similarity` - Minimum cosine similarity threshold (0.0-1.0)
    /// * `evidence_limit` - Maximum items per category (supporting/contradicting)
    /// * `contradiction_patterns` - Optional custom patterns indicating contradiction
    ///
    /// # Returns
    /// `VerificationResult` containing supporting/contradicting evidence and confidence score
    pub async fn run_hypothesis_verification(
        &self,
        hypothesis: &str,
        top_k: usize,
        min_similarity: f32,
        evidence_limit: usize,
        contradiction_patterns: Option<&[String]>,
    ) -> Result<Option<VerificationResult>> {
        let start = std::time::Instant::now();

        // Instrumentation: log setup
        if std::env::var("RUST_LOG")
            .unwrap_or_default()
            .contains("debug")
        {
            tracing::debug!(
                "hypothesis_verification_setup: ns={}, db={}, embedder_provider={}, embedder_model={}, embedder_dim={}, hypothesis_prefix={}, verify_top_k={}, min_similarity={}, evidence_limit={}",
                self.config.system.database_ns,
                self.config.system.database_db,
                self.get_embedding_metadata().0,
                self.get_embedding_metadata().1,
                self.get_embedding_metadata().2,
                &hypothesis[..hypothesis.len().min(50)],
                top_k,
                min_similarity,
                evidence_limit
            );
        }

        let embedding = self.embedder.embed(hypothesis).await?;
        let q_dim = embedding.len() as i64;

        let patterns = contradiction_patterns.unwrap_or(&[]).to_vec();
        let default_patterns: Vec<String> = CONTRADICTION_PATTERNS
            .iter()
            .map(|s| s.to_string())
            .collect();
        let all_patterns = if patterns.is_empty() {
            &default_patterns
        } else {
            &patterns
        };

        // Query KG entities and observations
        let query_sql = format!(
            "SELECT meta::id(id) as id, name, data, embedding FROM kg_entities \
             WHERE embedding_dim = $dim AND embedding IS NOT NULL LIMIT {}; \
             SELECT meta::id(id) as id, name, data, embedding FROM kg_observations \
             WHERE embedding_dim = $dim AND embedding IS NOT NULL LIMIT {};",
            top_k as i64, top_k as i64
        );

        if std::env::var("RUST_LOG")
            .unwrap_or_default()
            .contains("debug")
        {
            tracing::debug!(
                "hypothesis_verification_query: query_sql={}, dim={}, lim={}",
                query_sql,
                q_dim,
                top_k as i64
            );
        }

        let mut q = self
            .db
            .query(&query_sql)
            .bind(("dim", q_dim))
            .bind(("lim", top_k as i64))
            .await?;
        let mut rows: Vec<serde_json::Value> = q.take(0).unwrap_or_default();
        let mut rows2: Vec<serde_json::Value> = q.take(1).unwrap_or_default();
        rows.append(&mut rows2);

        let total_candidates = rows.len();

        if std::env::var("RUST_LOG")
            .unwrap_or_default()
            .contains("debug")
        {
            tracing::debug!(
                "hypothesis_verification_candidates: total_candidates_after_query={}",
                total_candidates
            );
        }

        let mut supporting = Vec::new();
        let mut contradicting = Vec::new();
        let mut matched_support = 0;
        let mut matched_contradict = 0;

        let mut candidates_with_embedding = 0;
        let mut candidates_after_similarity = 0;

        for r in rows {
            if let (Some(id), Some(name)) = (
                r.get("id").and_then(|v| v.as_str()),
                r.get("name").and_then(|v| v.as_str()),
            ) {
                let data = r.get("data");
                let text = Self::build_kg_text(name, data);

                // Embed the text if needed, but for now assume we have embedding or skip
                // For simplicity, check if embedding exists; if not, compute and persist
                let mut emb_opt = None;
                if let Some(ev) = r.get("embedding").and_then(|v| v.as_array()) {
                    let vecf: Vec<f32> = ev
                        .iter()
                        .filter_map(|x| x.as_f64())
                        .map(|f| f as f32)
                        .collect();
                    if vecf.len() == embedding.len() {
                        emb_opt = Some(vecf);
                        candidates_with_embedding += 1;
                    }
                }
                if emb_opt.is_none() {
                    let new_emb = self.embedder.embed(&text).await?;
                    if new_emb.len() == embedding.len() {
                        emb_opt = Some(new_emb.clone());
                        // Persist (similar to inject_memories)
                    }
                }
                if let Some(emb_e) = emb_opt {
                    let sim = Self::cosine_similarity(&embedding, &emb_e);
                    if sim >= min_similarity {
                        candidates_after_similarity += 1;
                        let item = EvidenceItem {
                            table: if id.starts_with("kg_entities:") {
                                "kg_entities"
                            } else {
                                "kg_observations"
                            }
                            .to_string(),
                            id: id.to_string(),
                            text: text.clone(),
                            similarity: sim,
                            provenance: data.cloned(),
                        };
                        let lower_text = text.to_lowercase();
                        let is_contradiction = all_patterns
                            .iter()
                            .any(|pat| lower_text.contains(&pat.to_lowercase()));
                        if is_contradiction {
                            contradicting.push(item);
                            matched_contradict += 1;
                        } else {
                            supporting.push(item);
                            matched_support += 1;
                        }
                    }
                }
            }
        }

        if std::env::var("RUST_LOG")
            .unwrap_or_default()
            .contains("debug")
        {
            tracing::debug!(
                "hypothesis_verification_counts: candidates_with_embedding={}, candidates_after_similarity={}",
                candidates_with_embedding,
                candidates_after_similarity
            );
        }

        // Sort and limit
        supporting.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        contradicting.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        supporting.truncate(evidence_limit);
        contradicting.truncate(evidence_limit);

        let total = supporting.len() + contradicting.len();
        let confidence_score = if total > 0 {
            supporting.len() as f32 / total as f32
        } else {
            0.5
        };

        let suggested_revision = if confidence_score < 0.4 {
            Some(format!(
                "Consider revising hypothesis based on {} contradicting items",
                contradicting.len()
            ))
        } else {
            None
        };

        let telemetry = json!({
            "embedding_dim": embedding.len(),
            "provider": self.get_embedding_metadata().0,
            "model": self.get_embedding_metadata().1,
            "dim": self.get_embedding_metadata().2,
            "k": top_k,
            "min_similarity": min_similarity,
            "time_ms": start.elapsed().as_millis(),
            "matched_support": matched_support,
            "matched_contradict": matched_contradict,
            "total_candidates": total_candidates,
            "candidates_with_embedding": candidates_with_embedding,
            "candidates_after_similarity": candidates_after_similarity
        });

        let result = VerificationResult {
            hypothesis: hypothesis.to_string(),
            supporting,
            contradicting,
            confidence_score,
            suggested_revision,
            telemetry,
        };

        Ok(Some(result))
    }
}
