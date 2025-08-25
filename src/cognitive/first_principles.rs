//! First Principles framework heuristic.

use super::framework::{Framework, top_keywords};
use super::types::FrameworkOutput;

pub struct FirstPrinciples;

impl Framework for FirstPrinciples {
    fn name(&self) -> &'static str {
        "FirstPrinciples"
    }

    fn analyze(&self, input: &str) -> FrameworkOutput {
        let kws = top_keywords(input, 3);
        let mut insights = Vec::new();
        for kw in kws.iter() {
            insights.push(format!("Reduce '{}' to primitives", kw));
        }
        if insights.is_empty() {
            insights.push("State irreducible constraints".to_string());
        }
        let questions = vec!["What must be true regardless of method?".to_string()];
        let next_steps = vec!["Express problem as variables and constraints".to_string()];
        FrameworkOutput {
            insights,
            questions,
            next_steps,
            meta: Default::default(),
        }
    }
}
