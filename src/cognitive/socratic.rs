//! Socratic framework heuristic.

use super::framework::{Framework, split_sentences};
use super::types::FrameworkOutput;

pub struct Socratic;

impl Framework for Socratic {
    fn name(&self) -> &'static str {
        "Socratic"
    }

    fn analyze(&self, input: &str) -> FrameworkOutput {
        let sents = split_sentences(input);
        let mut questions = Vec::new();
        for s in sents.iter().take(3) {
            let trimmed = s.trim().trim_end_matches(['.', '!', '?']);
            if !trimmed.is_empty() {
                questions.push(format!("What makes '{}' true?", trimmed));
            }
        }
        if questions.is_empty() {
            questions.push("What assumption am I making?".to_string());
        }
        let insights = vec!["Seek counterexamples and clarifications".to_string()];
        let next_steps = vec!["List assumptions and test one".to_string()];
        FrameworkOutput {
            insights,
            questions,
            next_steps,
            meta: Default::default(),
        }
    }
}
