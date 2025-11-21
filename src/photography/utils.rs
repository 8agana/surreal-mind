// Extracted from src/bin/photography.rs
// Helper functions for photography module

use anyhow::Result;
use strsim::jaro_winkler;
use surrealdb::{Surreal, engine::remote::ws::Client};

/// Formats a family ID for use in SurrealDB queries.
/// Ensures underscores instead of spaces, and backticks if non-alphanumeric characters are present.
pub fn format_family_id(last_name: &str) -> String {
    let lower = last_name.to_lowercase().replace(" ", "_");
    // Check for non-alphanumeric characters (excluding underscore)
    if lower.chars().any(|c| !c.is_alphanumeric() && c != '_') {
        format!("family:`{}`", lower)
    } else {
        format!("family:{}", lower)
    }
}

/// Parses skater names from a string, handling families, synchro, and multiple skaters.
pub fn parse_skater_names(name: &str) -> anyhow::Result<super::models::ParsedName> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow::anyhow!("Empty skater name"));
    }

    // Check if it's synchro
    if name.to_lowercase().starts_with("synchro ") {
        let team = name[8..].trim();
        let skater = super::models::ParsedSkater {
            first_name: "Synchro".to_string(),
            last_name: team.to_string(),
            _family_email: None,
        };
        return Ok(super::models::ParsedName {
            skaters: vec![skater],
            is_family: false,
            _is_synchro: true,
        });
    }

    // Split into words
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.is_empty() {
        return Err(anyhow::anyhow!("Empty skater name"));
    }

    // Last word is last_name
    let last_name = words.last().unwrap().to_string();

    // First part is all except last word
    let first_part = &name[..name.len() - last_name.len()].trim();

    // Parse first_part
    let first_names: Vec<String> = first_part
        .split(',')
        .flat_map(|s| s.split(" and "))
        .map(|s| s.trim().trim_end_matches(','))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    if first_names.is_empty() {
        return Err(anyhow::anyhow!("No first names found"));
    }

    let skaters: Vec<super::models::ParsedSkater> = first_names
        .into_iter()
        .map(|first| super::models::ParsedSkater {
            first_name: first,
            last_name: last_name.clone(),
            _family_email: None,
        })
        .collect();

    let is_family = skaters.len() > 1;

    Ok(super::models::ParsedName {
        skaters,
        is_family,
        _is_synchro: false,
    })
}

/// Converts a competition name to a valid ID by lowercasing and replacing special characters.
pub fn competition_to_id(competition: &str) -> String {
    competition
        .to_lowercase()
        .replace(" ", "_")
        .replace(",", "")
        .replace("-", "_")
}

/// Resolves a competition name using exact match, substring match, or fuzzy matching.
/// Returns the canonical competition name or an error with suggestions.
pub async fn resolve_competition(db: &Surreal<Client>, input: &str) -> Result<String> {
    // Query all competition names
    let mut resp = db.query("SELECT name FROM competition").await?;
    let competitions: Vec<String> = resp.take(0)?;

    if competitions.is_empty() {
        return Err(anyhow::anyhow!("No competitions found in database"));
    }

    let input_lower = input.to_lowercase();
    let mut exact_matches = Vec::new();

    // Strategy 1: Exact match or substring (case-insensitive)
    for comp in &competitions {
        let comp_lower = comp.to_lowercase();
        if comp_lower == input_lower || comp_lower.contains(&input_lower) {
            exact_matches.push(comp.clone());
        }
    }

    if exact_matches.len() == 1 {
        return Ok(exact_matches[0].clone());
    } else if exact_matches.len() > 1 {
        let names = exact_matches
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(anyhow::anyhow!(
            "Ambiguous input '{}'. Multiple matches: {}",
            input,
            names
        ));
    }

    // Strategy 2: Fuzzy matching with Jaro-Winkler
    let mut best_score = 0.0;

    for comp in &competitions {
        let score = jaro_winkler(&comp.to_lowercase(), &input_lower);
        if score > best_score {
            best_score = score;
        }
    }

    if best_score > 0.8 {
        // Check for multiple high matches
        let mut high_matches = Vec::new();
        for comp in &competitions {
            let score = jaro_winkler(&comp.to_lowercase(), &input_lower);
            if (score - best_score).abs() < 0.000001 {
                high_matches.push(comp.clone());
            }
        }
        if high_matches.len() == 1 {
            return Ok(high_matches[0].clone());
        } else {
            let names = high_matches
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(anyhow::anyhow!(
                "Ambiguous fuzzy matches for '{}'. Suggestions: {}",
                input,
                names
            ));
        }
    }

    // No match
    let available = competitions
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Err(anyhow::anyhow!(
        "No competition found matching '{}'. Available competitions: {}",
        input,
        available
    ))
}
