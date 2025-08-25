//! Root Cause analysis heuristic.

use super::framework::{Framework, split_sentences};
use super::types::FrameworkOutput;

pub struct RootCause;

impl Framework for RootCause {
    fn name(&self) -> &'static str {
        "RootCause"
    }

    fn analyze(&self, input: &str) -> FrameworkOutput {
        let sents = split_sentences(input);
        let mut insights = Vec::new();
        let mut questions = Vec::new();
        for (i, s) in sents.iter().take(2).enumerate() {
            insights.push(format!("Why-{} candidate: {}", i + 1, s));
            questions.push(format!("Why does '{}' happen?", s.trim()));
        }
        if questions.is_empty() {
            questions.push("What is the hidden constraint?".to_string());
        }
        let next_steps = vec!["Run 5-Whys on the top symptom".to_string()];
        FrameworkOutput {
            insights,
            questions,
            next_steps,
            meta: Default::default(),
        }
    }
}
