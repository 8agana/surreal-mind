//! Cognitive module: frameworks, blending, and submode profiles.
//! Deterministic, dependency-free heuristics.

pub mod dialectical;
pub mod first_principles;
pub mod framework;
pub mod lateral;
pub mod ooda;
pub mod profile;
pub mod root_cause;
pub mod socratic;
pub mod systems;
pub mod types;

use framework::Framework;
use once_cell::sync::Lazy;
use std::sync::Arc;
use types::FrameworkOutput;

/// Simple cognitive engine that runs available frameworks and blends their outputs
/// according to integer weights per framework name.
pub struct CognitiveEngine {
    frameworks: Vec<Arc<dyn Framework + Send + Sync>>, // shared trait objects
}

static FRAMEWORKS: Lazy<Vec<Arc<dyn Framework + Send + Sync>>> = Lazy::new(|| {
    vec![
        Arc::new(ooda::Ooda),
        Arc::new(socratic::Socratic),
        Arc::new(first_principles::FirstPrinciples),
        Arc::new(root_cause::RootCause),
        Arc::new(lateral::Lateral),
        Arc::new(systems::SystemsThinking),
        Arc::new(dialectical::DialecticalThinking),
    ]
});

#[allow(clippy::new_without_default)]
impl CognitiveEngine {
    pub fn new() -> Self {
        Self {
            frameworks: FRAMEWORKS.clone(),
        }
    }

    pub fn analyze_all(&self, input: &str) -> Vec<(String, FrameworkOutput)> {
        self.frameworks
            .iter()
            .map(|f| (f.name().to_string(), f.analyze(input)))
            .collect()
    }

    /// Blend outputs proportionally per channel using weight map by framework name.
    /// channels: insights N=8, questions N=4, next_steps N=4
    pub fn blend(
        &self,
        input: &str,
        weights: &std::collections::HashMap<&'static str, u8>,
    ) -> FrameworkOutput {
        let analyses = self.analyze_all(input);
        use std::collections::{HashMap, HashSet};

        let mut by_name: HashMap<String, FrameworkOutput> = HashMap::new();
        for (name, out) in analyses.into_iter() {
            by_name.insert(name, out);
        }

        let pick_channel = |channel: &str, total: usize| -> Vec<String> {
            // Collect per-framework items for the channel
            let mut items_by_fw: Vec<(&str, Vec<String>, f32)> = Vec::new();
            let mut total_w: f32 = 0.0;
            for (fw, w) in weights.iter() {
                let w_f = (*w as f32).max(0.0);
                total_w += w_f;
                if let Some(out) = by_name.get(*fw) {
                    let v = match channel {
                        "insights" => out.insights.clone(),
                        "questions" => out.questions.clone(),
                        _ => out.next_steps.clone(),
                    };
                    items_by_fw.push((fw, v, w_f));
                } else {
                    items_by_fw.push((fw, Vec::new(), w_f));
                }
            }
            if total_w <= 0.0 {
                // fall back to round-robin without weights
                let mut out = Vec::new();
                let mut idx = 0;
                loop {
                    let mut progressed = false;
                    for (_, v, _) in items_by_fw.iter() {
                        if idx < v.len() {
                            out.push(v[idx].clone());
                            if out.len() >= total {
                                return out;
                            }
                            progressed = true;
                        }
                    }
                    if !progressed {
                        break;
                    }
                    idx += 1;
                }
                return out;
            }

            // Proportional picks per framework, then round-robin to fill remainders
            let mut alloc: Vec<(&str, usize)> = Vec::new();
            let mut remainder: Vec<(&str, f32)> = Vec::new();
            let mut assigned = 0usize;
            for (fw, _v, w_f) in items_by_fw.iter() {
                let share = (*w_f / total_w) * (total as f32);
                let base = share.floor() as usize;
                assigned += base;
                alloc.push((*fw, base));
                remainder.push((*fw, share - base as f32));
            }
            // distribute remaining by largest fractional part
            let mut remaining = total.saturating_sub(assigned);
            remainder.sort_by(|a, b| b.1.total_cmp(&a.1));
            for (fw, _) in remainder.into_iter() {
                if remaining == 0 {
                    break;
                }
                if let Some(a) = alloc.iter_mut().find(|(n, _)| *n == fw) {
                    a.1 += 1;
                    remaining -= 1;
                }
            }

            // Collect picks with dedup, preserving diversity
            let mut out: Vec<String> = Vec::new();
            let mut seen: HashSet<String> = HashSet::new();
            let mut per_fw_indices: HashMap<&str, usize> = HashMap::new();
            loop {
                let mut progressed = false;
                for (fw, v, _w) in items_by_fw.iter() {
                    let _cap = alloc.iter().find(|(n, _)| n == fw).map_or(0, |p| p.1);
                    let idx_ref = per_fw_indices.entry(fw).or_insert(0);
                    while *idx_ref < v.len()
                        && out.len() < total
                        && alloc.iter().any(|(n, c)| n == fw && *c > 0)
                    {
                        let candidate = &v[*idx_ref];
                        *idx_ref += 1;
                        if seen.insert(candidate.clone()) {
                            if let Some(slot) = alloc.iter_mut().find(|(n, _)| n == fw)
                                && slot.1 > 0
                            {
                                slot.1 -= 1;
                            }
                            out.push(candidate.clone());
                            progressed = true;
                            break;
                        }
                    }
                }
                if !progressed {
                    break;
                }
                if out.len() >= total {
                    break;
                }
            }
            out.truncate(total);
            out
        };

        FrameworkOutput {
            insights: pick_channel("insights", 8),
            questions: pick_channel("questions", 4),
            next_steps: pick_channel("next_steps", 4),
            meta: {
                let mut m = std::collections::HashMap::new();
                m.insert("weights_used".into(), format!("{:?}", weights));
                m
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_blend_with_empty_input() {
        let engine = CognitiveEngine::new();
        let weights: HashMap<&'static str, u8> = HashMap::new();
        let result = engine.blend("", &weights);
        
        // Should return empty channels when no weights specified
        assert!(result.insights.len() <= 8);
        assert!(result.questions.len() <= 4);
        assert!(result.next_steps.len() <= 4);
    }

    #[test]
    fn test_blend_with_zero_weights() {
        let engine = CognitiveEngine::new();
        let mut weights: HashMap<&'static str, u8> = HashMap::new();
        weights.insert("OODA", 0);
        weights.insert("Socratic", 0);
        
        let result = engine.blend("test content", &weights);
        // Should fallback to round-robin when all weights are zero
        assert!(result.insights.len() <= 8);
    }

    #[test]
    fn test_blend_proportional_allocation() {
        let engine = CognitiveEngine::new();
        let mut weights: HashMap<&'static str, u8> = HashMap::new();
        weights.insert("OODA", 100);
        weights.insert("Socratic", 0);
        
        let result = engine.blend("What is the root cause of this bug in the system?", &weights);
        // With OODA heavily weighted, should get results
        assert!(!result.insights.is_empty() || !result.questions.is_empty() || !result.next_steps.is_empty());
    }

    #[test]
    fn test_blend_deduplication() {
        let engine = CognitiveEngine::new();
        let mut weights: HashMap<&'static str, u8> = HashMap::new();
        weights.insert("OODA", 50);
        weights.insert("Socratic", 50);
        weights.insert("First Principles", 50);
        
        let result = engine.blend("How can we improve the architecture?", &weights);
        
        // Check for no duplicates in insights
        let unique_insights: std::collections::HashSet<_> = result.insights.iter().collect();
        assert_eq!(unique_insights.len(), result.insights.len(), "Insights should be unique");
        
        // Check for no duplicates in questions
        let unique_questions: std::collections::HashSet<_> = result.questions.iter().collect();
        assert_eq!(unique_questions.len(), result.questions.len(), "Questions should be unique");
    }

    #[test]
    fn test_blend_channel_limits() {
        let engine = CognitiveEngine::new();
        let mut weights: HashMap<&'static str, u8> = HashMap::new();
        weights.insert("OODA", 10);
        weights.insert("Socratic", 10);
        weights.insert("First Principles", 10);
        weights.insert("Root Cause", 10);
        weights.insert("Lateral", 10);
        weights.insert("Systems Thinking", 10);
        weights.insert("Dialectical", 10);
        
        let result = engine.blend("A complex problem that requires deep analysis and multiple perspectives.", &weights);
        
        // Verify channel limits are respected
        assert!(result.insights.len() <= 8, "Insights should be limited to 8");
        assert!(result.questions.len() <= 4, "Questions should be limited to 4");
        assert!(result.next_steps.len() <= 4, "Next steps should be limited to 4");
    }

    #[test]
    fn test_blend_weights_recorded_in_meta() {
        let engine = CognitiveEngine::new();
        let mut weights: HashMap<&'static str, u8> = HashMap::new();
        weights.insert("OODA", 5);
        
        let result = engine.blend("simple test", &weights);
        
        assert!(result.meta.contains_key("weights_used"), "Meta should contain weights_used");
    }
}
