//! Lateral Thinking heuristic.

use super::framework::{Framework, top_keywords};
use super::types::FrameworkOutput;

pub struct Lateral;

impl Framework for Lateral {
    fn name(&self) -> &'static str {
        "Lateral"
    }

    fn analyze(&self, input: &str) -> FrameworkOutput {
        let kws = top_keywords(input, 2);
        let mut insights = Vec::new();
        if kws.len() >= 2 {
            insights.push(format!("Force analogy: '{}' as '{}'", kws[0], kws[1]));
            insights.push(format!("Invert: what if '{}' was forbidden?", kws[0]));
        } else {
            insights.push("Try inversion and random word association".to_string());
        }
        let questions = vec!["What orthogonal domain has solved this?".to_string()];
        let next_steps = vec!["Generate 3 analogies and test one".to_string()];
        FrameworkOutput {
            insights,
            questions,
            next_steps,
            meta: Default::default(),
        }
    }
}
