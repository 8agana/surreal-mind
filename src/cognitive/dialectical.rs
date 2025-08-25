//! Dialectical Thinking heuristic.

use super::framework::{Framework, split_sentences};
use super::types::FrameworkOutput;

pub struct DialecticalThinking;

impl Framework for DialecticalThinking {
    fn name(&self) -> &'static str {
        "Dialectical"
    }

    fn analyze(&self, input: &str) -> FrameworkOutput {
        let sents = split_sentences(input);
        let mut thesis = String::new();
        let mut antithesis = String::new();
        if let Some(s) = sents.first() {
            thesis = s.clone();
        }
        if let Some(s) = sents.get(1) {
            antithesis = s.clone();
        }
        if antithesis.is_empty() {
            antithesis = "Consider the opposite constraint".to_string();
        }
        let insights = vec![
            format!("Thesis: {}", thesis),
            format!("Antithesis: {}", antithesis),
            "Synthesis: preserve strengths, remove contradictions".to_string(),
        ];
        let questions = vec!["What synthesis keeps both benefits?".to_string()];
        let next_steps = vec!["Draft a synthesis hypothesis and test".to_string()];
        FrameworkOutput {
            insights,
            questions,
            next_steps,
            meta: Default::default(),
        }
    }
}
