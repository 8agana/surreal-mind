use super::{ConvoData, ConvoOpts, FrameworkEnvelope};
use anyhow::{Result, anyhow};
use blake3;
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

// Lazy-loaded lexicons from env, fallback to defaults
static LEXICON_DECIDE: LazyLock<Vec<String>> = LazyLock::new(|| {
    std::env::var("SURR_THINK_LEXICON_DECIDE")
        .unwrap_or("decide,ship,fix,choose,implement,deploy,select,finalize".to_string())
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .collect()
});

static LEXICON_VENT: LazyLock<Vec<String>> = LazyLock::new(|| {
    std::env::var("SURR_THINK_LEXICON_VENT")
        .unwrap_or("hate,pissed,broken,fuck,shit,sucks,awful".to_string())
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .collect()
});

static LEXICON_POS: LazyLock<Vec<String>> = LazyLock::new(|| {
    std::env::var("SURR_THINK_LEXICON_POS")
        .unwrap_or("great,good,love,nice,excited".to_string())
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .collect()
});

static LEXICON_NEG: LazyLock<Vec<String>> = LazyLock::new(|| {
    std::env::var("SURR_THINK_LEXICON_NEG")
        .unwrap_or("bad,broken,hate,stuck,fuck,shit".to_string())
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .collect()
});

static LEXICON_CAUSAL: LazyLock<Vec<String>> = LazyLock::new(|| {
    std::env::var("SURR_THINK_LEXICON_CAUSAL")
        .unwrap_or("because,why,root,reason,due,constraint,risk,block,cause".to_string())
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .collect()
});

#[derive(Debug, Clone, PartialEq)]
pub struct ConvoSense {
    pub intent_polarity: String,
    pub valence: String,
    pub complexity: String,
    pub stalled: bool,
    pub causal_count: usize,
}

fn normalize(content: &str) -> String {
    content
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn analyze(content: &str) -> ConvoSense {
    let tokens: Vec<&str> = content.split_whitespace().collect();
    let token_count = tokens.len();
    let unique: HashSet<&str> = tokens.iter().cloned().collect();
    let unique_ratio = unique.len() as f32 / token_count.max(1) as f32;

    // Intent polarity
    let has_decide = LEXICON_DECIDE.iter().any(|w| content.contains(w));
    let has_should_need = content.contains("should") && (content.contains("need") || has_decide);
    let intent = if has_decide || has_should_need {
        "decide"
    } else {
        let has_vent = LEXICON_VENT.iter().any(|w| content.contains(w));
        let exclamations = content.chars().filter(|&c| c == '!').count() > 0;
        if has_vent && exclamations {
            "vent"
        } else {
            "explore"
        }
    };

    // Valence
    let pos_count = LEXICON_POS
        .iter()
        .map(|w| content.matches(w).count())
        .sum::<usize>();
    let neg_count = LEXICON_NEG
        .iter()
        .map(|w| content.matches(w).count())
        .sum::<usize>();
    let score = pos_count as i32 - neg_count as i32;
    let valence = match score.cmp(&0) {
        std::cmp::Ordering::Greater => "positive",
        std::cmp::Ordering::Less => "negative",
        std::cmp::Ordering::Equal => "neutral",
    };

    // Complexity and causal
    let causal_count = LEXICON_CAUSAL
        .iter()
        .map(|w| content.matches(w).count())
        .sum::<usize>();
    let complexity = if token_count > 60 || (unique_ratio > 0.6 && causal_count >= 2) {
        "high"
    } else if token_count > 25 {
        "med"
    } else {
        "low"
    };

    // Stalled
    let stalled = token_count < 6 || (valence == "neutral" && token_count < 10);

    ConvoSense {
        intent_polarity: intent.to_string(),
        valence: valence.to_string(),
        complexity: complexity.to_string(),
        stalled,
        causal_count,
    }
}

fn select_methodology(sense: &ConvoSense) -> &'static str {
    if sense.intent_polarity == "vent" && sense.valence == "negative" {
        "mirroring"
    } else if sense.intent_polarity == "decide"
        && (sense.complexity == "high" || sense.causal_count >= 2)
    {
        "first_principles"
    } else if sense.intent_polarity == "explore" && sense.complexity == "high" {
        "socratic"
    } else if sense.stalled {
        "lateral"
    } else {
        "constraints"
    }
}

fn one_line_summary(content: &str) -> String {
    let cleaned = Regex::new(r"[^\w\s]")
        .expect("regex should compile")
        .replace_all(content, "");
    let words: Vec<&str> = cleaned.split_whitespace().collect();
    if words.len() < 5 {
        return content.chars().take(100).collect();
    }
    // Simple: take first 10 words, or try to find verb + object
    let start = words
        .iter()
        .position(|w| ["need", "want", "decide"].contains(w));
    let slice = if let Some(idx) = start {
        &words[idx..(idx + 10).min(words.len())]
    } else {
        &words[..10.min(words.len())]
    };
    let summary = slice.join(" ") + if words.len() > 10 { "..." } else { "" };
    summary.chars().take(100).collect()
}

fn generate_convo_data(
    method: &str,
    content: &str,
    seed: u64,
    tag_whitelist: &[String],
) -> ConvoData {
    let mut rng = seed;
    let mut idx = |n: usize| {
        rng = rng.wrapping_add(1);
        (rng % n as u64) as usize
    };

    let summary = one_line_summary(content)
        .chars()
        .take(140)
        .collect::<String>();

    let (takeaways, prompts, next_step, tags) = match method {
        "socratic" => {
            let takes = vec![
                "Assumption to test: the core belief underlying this".to_string(),
                "Key term to define: clarity on ambiguous concept".to_string(),
            ][..2.min(idx(3) + 1)]
                .to_vec();
            let proms = vec![
                "What would make this obviously wrong?".to_string(),
                "Which constraint matters most?".to_string(),
            ][..2.min(idx(2) + 1)]
                .to_vec();
            let next = vec![
                "Write one crisp question and answer it in 2-3 lines.".to_string(),
                "Formulate a single key question and test it briefly.".to_string(),
            ][idx(2)]
            .clone();
            (
                takes,
                proms,
                next,
                vec!["plan".to_string(), "dx".to_string()],
            )
        }
        "first_principles" => {
            let takes = vec![
                format!(
                    "Problem reduction: {}",
                    content.chars().take(50).collect::<String>()
                ),
                "Key variable to measure: identify what to track".to_string(),
            ];
            let proms = vec!["If you removed X, what remains?".to_string()];
            let next = vec![
                "List 2 primitives and instrument one.".to_string(),
                "Break down to fundamentals and measure a key variable.".to_string(),
            ][idx(2)]
            .clone();
            (
                takes,
                proms,
                next,
                vec!["debug".to_string(), "plan".to_string()],
            )
        }
        "mirroring" => {
            let takes = vec![
                "Reflection: acknowledge the frustration".to_string(),
                "Stabilizer: take a deep breath or set a boundary".to_string(),
            ];
            let proms = vec!["What would feeling 10% better look like?".to_string()];
            let next = vec![
                "The smallest stabilizing action in 5-10 min.".to_string(),
                "One small step to regain control.".to_string(),
            ][idx(2)]
            .clone();
            (takes, proms, next, vec!["convo".to_string()])
        }
        "lateral" => {
            let takes = vec![
                "Analogous domain: e.g., shipping vs. software".to_string(),
                "Borrowed constraint: apply from another field".to_string(),
            ];
            let proms = vec!["What is the opposite lever?".to_string()];
            let next = vec![
                "Try a tiny experiment from the analogy.".to_string(),
                "Experiment with an idea from the adjacent concept.".to_string(),
            ][idx(2)]
            .clone();
            (takes, proms, next, vec!["idea".to_string()])
        }
        _ => {
            // constraints
            let takes = vec![
                "Top constraint: identify the bottleneck".to_string(),
                "Top lever: what amplifies change".to_string(),
            ];
            let proms = vec!["What single change increases the lever?".to_string()];
            let next = vec![
                "Apply the lever in a 15-min block.".to_string(),
                "Test one change to move the lever.".to_string(),
            ][idx(2)]
            .clone();
            (takes, proms, next, vec!["plan".to_string()])
        }
    };

    // Tag merge
    let whitelist: HashSet<String> = tag_whitelist.iter().cloned().collect();
    let merged_tags: Vec<String> = tags.into_iter().filter(|t| whitelist.contains(t)).collect();

    ConvoData {
        summary,
        takeaways,
        prompts,
        next_step: next_step.chars().take(140).collect(),
        tags: merged_tags,
    }
}

fn validate(data: &ConvoData, strict: bool) -> Result<()> {
    if (data.takeaways.len() > 2 || data.prompts.len() > 2 || data.summary.len() > 140) && strict {
        return Err(anyhow!(
            "Exceeds limits: takeaways={}, prompts={}, summary_len={}",
            data.takeaways.len(),
            data.prompts.len(),
            data.summary.len()
        ));
    }
    Ok(())
}

pub fn run_convo_impl(content: &str, opts: &ConvoOpts) -> Result<FrameworkEnvelope<ConvoData>> {
    let norm = normalize(content);
    let sense = analyze(&norm);
    let method = select_methodology(&sense);
    let hash = blake3::hash(norm.as_bytes());
    let seed = u64::from_le_bytes(
        hash.as_bytes()[0..8]
            .try_into()
            .expect("blake3 hash should be at least 8 bytes"),
    );
    let data = generate_convo_data(method, &norm, seed, &opts.tag_whitelist);
    validate(&data, opts.strict_json)?;
    Ok(FrameworkEnvelope {
        framework_version: "convo/1".to_string(),
        methodology: method.to_string(),
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize() {
        assert_eq!(normalize("  Hello   World!  "), "hello world!");
    }

    #[test]
    fn test_analyze_decide() {
        let sense = analyze("I need to decide what to implement next.");
        assert_eq!(sense.intent_polarity, "decide");
        assert_eq!(sense.complexity, "low");
    }

    #[test]
    fn test_analyze_vent() {
        let sense = analyze("This is broken and I hate it!");
        assert_eq!(sense.intent_polarity, "vent");
        assert_eq!(sense.valence, "negative");
    }

    #[test]
    fn test_analyze_high_complexity() {
        let long = "Why because reason due constraint risk block cause why because reason due constraint risk block cause".repeat(4);
        let sense = analyze(&long);
        assert_eq!(sense.complexity, "high");
    }

    #[test]
    fn test_select_methodology() {
        let sense = ConvoSense {
            intent_polarity: "vent".to_string(),
            valence: "negative".to_string(),
            complexity: "low".to_string(),
            stalled: false,
            causal_count: 0,
        };
        assert_eq!(select_methodology(&sense), "mirroring");
    }

    #[test]
    fn test_stability_seed() {
        let opts = ConvoOpts {
            strict_json: false,
            tag_whitelist: vec!["plan".to_string()],
            timeout_ms: 600,
        };
        let env1 = run_convo_impl("test content stable", &opts).unwrap();
        let env2 = run_convo_impl("test content stable", &opts).unwrap();
        assert_eq!(env1.data.takeaways, env2.data.takeaways); // Deterministic
    }

    #[test]
    fn test_neutral_constraints() {
        let sense = ConvoSense {
            intent_polarity: "explore".to_string(),
            valence: "neutral".to_string(),
            complexity: "low".to_string(),
            stalled: false,
            causal_count: 0,
        };
        assert_eq!(select_methodology(&sense), "constraints");
    }

    #[test]
    fn test_validation_strict() {
        let opts = ConvoOpts {
            strict_json: true,
            tag_whitelist: vec![],
            timeout_ms: 600,
        };
        let data = ConvoData {
            summary: "short".to_string(),
            takeaways: vec!["a".to_string(), "b".to_string(), "c".to_string()], // exceeds
            prompts: vec![],
            next_step: "step".to_string(),
            tags: vec![],
        };
        let result = validate(&data, opts.strict_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_tag_whitelist_merge() {
        let opts = ConvoOpts {
            strict_json: false,
            tag_whitelist: vec!["plan".to_string(), "debug".to_string()], // excludes "idea"
            timeout_ms: 600,
        };
        let envelope = run_convo_impl("test content for tagging", &opts).unwrap();
        // Lateral method includes "idea", but whitelist excludes it
        assert!(!envelope.data.tags.contains(&"idea".to_string()));
        // Should include allowed tags
        assert!(
            envelope
                .data
                .tags
                .iter()
                .all(|t| opts.tag_whitelist.contains(t))
        );
    }

    #[test]
    fn test_strict_limits_drop() {
        let opts = ConvoOpts {
            strict_json: true,
            tag_whitelist: vec!["plan".into()],
            timeout_ms: 200,
        };
        let data = ConvoData {
            summary: "x".repeat(141),                            // over 140
            takeaways: vec!["a".into(), "b".into(), "c".into()], // >2
            prompts: vec!["p1".into()],
            next_step: "step".into(),
            tags: vec![],
        };
        assert!(super::validate(&data, opts.strict_json).is_err());
    }
}
