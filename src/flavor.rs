//! Flavor tagging for thoughts based on content keywords

/// Deterministic flavor tags for thought content
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flavor {
    Contrarian,
    Abstract,
    Emotional,
    Solution,
    Neutral,
}

impl Flavor {
    pub fn as_str(&self) -> &'static str {
        match self {
            Flavor::Contrarian => "contrarian",
            Flavor::Abstract => "abstract",
            Flavor::Emotional => "emotional",
            Flavor::Solution => "solution",
            Flavor::Neutral => "neutral",
        }
    }
}

/// Determine flavor based on keyword presence
pub fn tag_flavor(content: &str) -> Flavor {
    let lower = content.to_lowercase();

    // Check in order of precedence
    if contains_contrarian_keywords(&lower) {
        Flavor::Contrarian
    } else if contains_abstract_keywords(&lower) {
        Flavor::Abstract
    } else if contains_emotional_keywords(&lower) {
        Flavor::Emotional
    } else if contains_solution_keywords(&lower) {
        Flavor::Solution
    } else {
        Flavor::Neutral
    }
}

fn contains_contrarian_keywords(text: &str) -> bool {
    let keywords = [
        "but",
        "however",
        "contradict",
        "fails",
        "wrong",
        "instead",
        "although",
        "despite",
    ];
    keywords.iter().any(|kw| text.contains(kw))
}

fn contains_abstract_keywords(text: &str) -> bool {
    let keywords = [
        "theory",
        "concept",
        "principle",
        "metaphysical",
        "abstract",
        "paradigm",
        "framework",
        "philosophical",
    ];
    keywords.iter().any(|kw| text.contains(kw))
}

fn contains_emotional_keywords(text: &str) -> bool {
    let keywords = [
        "feel", "trust", "care", "empathy", "hurt", "love", "fear", "happy", "sad", "angry",
    ];
    keywords.iter().any(|kw| text.contains(kw))
}

fn contains_solution_keywords(text: &str) -> bool {
    let keywords = [
        "fix",
        "solve",
        "implement",
        "design",
        "plan",
        "build",
        "create",
        "develop",
        "improve",
    ];
    keywords.iter().any(|kw| text.contains(kw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contrarian_flavor() {
        assert_eq!(tag_flavor("But that won't work"), Flavor::Contrarian);
        assert_eq!(
            tag_flavor("However, we should reconsider"),
            Flavor::Contrarian
        );
        assert_eq!(
            tag_flavor("This contradicts the assumption"),
            Flavor::Contrarian
        );
        assert_eq!(
            tag_flavor("The system fails under load"),
            Flavor::Contrarian
        );
    }

    #[test]
    fn test_abstract_flavor() {
        assert_eq!(
            tag_flavor("The theory behind this approach"),
            Flavor::Abstract
        );
        assert_eq!(
            tag_flavor("A fundamental principle of computing"),
            Flavor::Abstract
        );
        assert_eq!(tag_flavor("The concept is sound"), Flavor::Abstract);
        assert_eq!(
            tag_flavor("From a metaphysical perspective"),
            Flavor::Abstract
        );
    }

    #[test]
    fn test_emotional_flavor() {
        assert_eq!(tag_flavor("I feel this is important"), Flavor::Emotional);
        assert_eq!(
            tag_flavor("We need to trust the process"),
            Flavor::Emotional
        );
        assert_eq!(tag_flavor("I care about the outcome"), Flavor::Emotional);
        assert_eq!(tag_flavor("Show empathy to users"), Flavor::Emotional);
    }

    #[test]
    fn test_solution_flavor() {
        assert_eq!(tag_flavor("Let's fix this bug"), Flavor::Solution);
        assert_eq!(
            tag_flavor("We can solve it by refactoring"),
            Flavor::Solution
        );
        assert_eq!(tag_flavor("Implement the new feature"), Flavor::Solution);
        assert_eq!(tag_flavor("Design a better architecture"), Flavor::Solution);
    }

    #[test]
    fn test_neutral_fallback() {
        assert_eq!(tag_flavor("The weather is nice today"), Flavor::Neutral);
        assert_eq!(tag_flavor("Looking at the data"), Flavor::Neutral);
        assert_eq!(tag_flavor("This is a test"), Flavor::Neutral);
    }

    #[test]
    fn test_precedence() {
        // Contrarian takes precedence
        assert_eq!(
            tag_flavor("But we should implement a solution"),
            Flavor::Contrarian
        );
        // Abstract before emotional
        assert_eq!(
            tag_flavor("The theory makes me feel confident"),
            Flavor::Abstract
        );
        // Emotional before solution
        assert_eq!(tag_flavor("I trust we can fix this"), Flavor::Emotional);
    }
}
