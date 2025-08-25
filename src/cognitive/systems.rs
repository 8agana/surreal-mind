//! Systems Thinking heuristic.

use super::framework::{Framework, split_sentences, top_keywords};
use super::types::FrameworkOutput;

pub struct SystemsThinking;

impl Framework for SystemsThinking {
    fn name(&self) -> &'static str {
        "SystemsThinking"
    }

    fn analyze(&self, input: &str) -> FrameworkOutput {
        let sents = split_sentences(input);
        let kws = top_keywords(input, 4);
        let mut insights = Vec::new();
        if kws.len() >= 2 {
            insights.push(format!(
                "Feedback loop: {} -> {} -> {}",
                kws[0], kws[1], kws[0]
            ));
        }
        if let Some(s) = sents.first() {
            insights.push(format!("Stock/flow: '{}' accumulates then releases", s));
        }
        let questions = vec!["Where is the leverage point?".to_string()];
        let next_steps = vec!["Sketch a causal loop diagram".to_string()];
        FrameworkOutput {
            insights,
            questions,
            next_steps,
            meta: Default::default(),
        }
    }
}
