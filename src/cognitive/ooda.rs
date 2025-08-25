//! OODA framework heuristic.

use super::framework::{Framework, split_sentences, top_keywords};
use super::types::FrameworkOutput;

pub struct Ooda;

impl Framework for Ooda {
    fn name(&self) -> &'static str {
        "OODA"
    }

    fn analyze(&self, input: &str) -> FrameworkOutput {
        let sents = split_sentences(input);
        let kws = top_keywords(input, 5);
        let mut insights = Vec::new();
        insights.push(format!("Observe: key signals -> {}", kws.join(", ")));
        if let Some(first) = sents.first() {
            insights.push(format!("Orient: framing '{}'", first));
        }
        let questions = vec![
            "Decide: what is the immediate objective?".to_string(),
            "Act: what is the smallest next action?".to_string(),
        ];
        let next_steps = vec![
            "Define objective and stopping criteria".to_string(),
            "Take a minimal reversible step".to_string(),
        ];
        FrameworkOutput {
            insights,
            questions,
            next_steps,
            meta: Default::default(),
        }
    }
}
