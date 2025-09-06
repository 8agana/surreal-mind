//! Mathematical utility functions

/// Calculate cosine similarity between two vectors
/// Returns 0.0 if dimensions don't match
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        tracing::warn!(
            "cosine_similarity dimension mismatch: a={}, b={}",
            a.len(),
            b.len()
        );
        return 0.0;
    }
    if a.is_empty() {
        return 0.0;
    }
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot_product / (norm_a * norm_b)
}
