use crate::error::{Result, SurrealMindError};
use std::collections::HashMap;

/// Map of workspace aliases to absolute paths
#[derive(Debug, Clone)]
pub struct WorkspaceMap {
    aliases: HashMap<String, String>,
}

impl WorkspaceMap {
    /// Load workspace aliases from WORKSPACE_* environment variables
    pub fn from_env() -> Self {
        let mut aliases = HashMap::new();

        for (key, value) in std::env::vars() {
            if let Some(alias) = key.strip_prefix("WORKSPACE_") {
                // Convert to lowercase for case-insensitive matching
                let alias_lower = alias.to_lowercase();
                aliases.insert(alias_lower, value);
            }
        }

        Self { aliases }
    }

    /// List all available workspace aliases (sorted)
    pub fn list_aliases(&self) -> Vec<String> {
        let mut aliases: Vec<String> = self.aliases.keys().cloned().collect();
        aliases.sort();
        aliases
    }

    /// Get the path for an alias (case-insensitive lookup)
    pub fn get(&self, alias: &str) -> Option<&String> {
        self.aliases.get(&alias.to_lowercase())
    }

    /// Check if an alias exists
    pub fn contains(&self, alias: &str) -> bool {
        self.aliases.contains_key(&alias.to_lowercase())
    }
}

/// Resolve a workspace string to an absolute path
///
/// # Arguments
/// * `input` - User-provided cwd value (alias, absolute path, or tilde path)
/// * `map` - Workspace alias map loaded from environment
///
/// # Returns
/// * `Ok(String)` - Resolved absolute path
/// * `Err` - Invalid alias with suggestions
///
/// # Behavior
/// - If input starts with "/" or "~": treat as literal path (expand tilde if needed)
/// - Otherwise: treat as workspace alias and resolve from map
/// - If alias not found: return error with available aliases and closest match suggestion
pub fn resolve_workspace(input: &str, map: &WorkspaceMap) -> Result<String> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(SurrealMindError::InvalidParams {
            message: "cwd cannot be empty".into(),
        });
    }

    // Check if it's a literal path (absolute or tilde-prefixed)
    if trimmed.starts_with('/') {
        // Absolute path - return as-is
        return Ok(trimmed.to_string());
    }

    if trimmed.starts_with('~') {
        // Tilde expansion
        return expand_tilde(trimmed);
    }

    // Treat as workspace alias - lookup in map
    match map.get(trimmed) {
        Some(path) => Ok(path.clone()),
        None => {
            // Alias not found - generate helpful error message
            let available = map.list_aliases();
            let suggestion = find_closest_match(trimmed, &available);

            let mut msg = format!(
                "Unknown workspace alias: '{}'. Available workspaces: {}",
                trimmed,
                if available.is_empty() {
                    "none (set WORKSPACE_* env vars)".to_string()
                } else {
                    available.join(", ")
                }
            );

            if let Some(closest) = suggestion {
                msg.push_str(&format!(". Did you mean '{}'?", closest));
            }

            Err(SurrealMindError::InvalidParams { message: msg })
        }
    }
}

/// Expand tilde (~) in paths to home directory
fn expand_tilde(path: &str) -> Result<String> {
    if !path.starts_with('~') {
        return Ok(path.to_string());
    }

    let home = std::env::var("HOME").map_err(|_| SurrealMindError::InvalidParams {
        message: "Cannot expand '~': HOME environment variable not set".into(),
    })?;

    if path == "~" {
        Ok(home)
    } else if path.starts_with("~/") {
        Ok(path.replacen("~", &home, 1))
    } else {
        // ~username syntax not supported
        Err(SurrealMindError::InvalidParams {
            message: format!("Unsupported tilde expansion: '{}'. Use '~/' or absolute path.", path),
        })
    }
}

/// Find the closest matching alias using simple edit distance
fn find_closest_match(input: &str, candidates: &[String]) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }

    let input_lower = input.to_lowercase();

    // Find candidate with minimum Levenshtein distance
    let mut best_match: Option<(String, usize)> = None;

    for candidate in candidates {
        let distance = levenshtein_distance(&input_lower, &candidate.to_lowercase());

        match best_match {
            None => best_match = Some((candidate.clone(), distance)),
            Some((_, best_dist)) if distance < best_dist => {
                best_match = Some((candidate.clone(), distance));
            }
            _ => {}
        }
    }

    // Only suggest if edit distance is reasonable (< 4)
    best_match.and_then(|(alias, dist)| if dist < 4 { Some(alias) } else { None })
}

/// Compute Levenshtein distance between two strings
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();

    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();

    for (i, c1) in s1_chars.iter().enumerate() {
        for (j, c2) in s2_chars.iter().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            matrix[i + 1][j + 1] = std::cmp::min(
                std::cmp::min(
                    matrix[i][j + 1] + 1,     // deletion
                    matrix[i + 1][j] + 1,     // insertion
                ),
                matrix[i][j] + cost,          // substitution
            );
        }
    }

    matrix[len1][len2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_map_from_env() {
        unsafe {
            std::env::set_var("WORKSPACE_HOME", "/Users/test");
            std::env::set_var("WORKSPACE_PROJECTS", "/Users/test/Projects");
        }

        let map = WorkspaceMap::from_env();
        assert_eq!(map.get("home"), Some(&"/Users/test".to_string()));
        assert_eq!(map.get("HOME"), Some(&"/Users/test".to_string())); // case insensitive
        assert_eq!(map.get("projects"), Some(&"/Users/test/Projects".to_string()));

        unsafe {
            std::env::remove_var("WORKSPACE_HOME");
            std::env::remove_var("WORKSPACE_PROJECTS");
        }
    }

    #[test]
    fn test_resolve_absolute_path() {
        let map = WorkspaceMap { aliases: HashMap::new() };
        assert_eq!(
            resolve_workspace("/absolute/path", &map).unwrap(),
            "/absolute/path"
        );
    }

    #[test]
    fn test_resolve_tilde_expansion() {
        unsafe {
            std::env::set_var("HOME", "/Users/test");
        }
        let map = WorkspaceMap { aliases: HashMap::new() };

        assert_eq!(resolve_workspace("~/Projects", &map).unwrap(), "/Users/test/Projects");
        assert_eq!(resolve_workspace("~", &map).unwrap(), "/Users/test");

        unsafe {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn test_resolve_workspace_alias() {
        let mut aliases = HashMap::new();
        aliases.insert("surreal-mind".to_string(), "/path/to/surreal-mind".to_string());
        let map = WorkspaceMap { aliases };

        assert_eq!(
            resolve_workspace("surreal-mind", &map).unwrap(),
            "/path/to/surreal-mind"
        );
    }

    #[test]
    fn test_resolve_unknown_alias_with_suggestion() {
        let mut aliases = HashMap::new();
        aliases.insert("surreal-mind".to_string(), "/path/to/surreal-mind".to_string());
        let map = WorkspaceMap { aliases };

        let result = resolve_workspace("surreal", &map);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("surreal-mind")); // Suggestion
        assert!(err_msg.contains("Available workspaces"));
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("surreal", "surreal-mind"), 5);
        assert_eq!(levenshtein_distance("home", "home"), 0);
    }
}