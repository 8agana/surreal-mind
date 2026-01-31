# Task: Workspace Alias Resolution for Delegation Tools

**Status:** Ready for Implementation
**Priority:** Medium
**Estimated Effort:** 3-4 hours
**Created:** 2026-01-30

## Overview

Enable workspace aliases for the `cwd` parameter in delegation tools (`call_cc`, `call_gem`, `call_warp`, `call_vibe`) so users can use short names like `"surreal-mind"` instead of full paths like `"/Users/samuelatagana/Projects/LegacyMind/surreal-mind"`.

**User-facing benefit:**
```bash
# Before
call_cc(cwd="/Users/samuelatagana/Projects/LegacyMind/surreal-mind", prompt="...")

# After
call_cc(cwd="surreal-mind", prompt="...")
```

**Key features:**
- Load workspace aliases from `WORKSPACE_*` environment variables
- Resolve aliases at tool invocation time
- Backward compatible: absolute paths and tilde expansion still work
- Helpful error messages that suggest valid aliases on failure

---

## Implementation Plan

### Phase 1: Core Infrastructure (Workspace Resolution)

#### 1.1 Create `src/workspace.rs` (NEW FILE)

**Purpose:** Workspace alias mapping and resolution logic.

**Contents:**
```rust
use crate::error::{Result, SurrealMindError};
use std::collections::HashMap;
use std::path::PathBuf;

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
        std::env::set_var("WORKSPACE_HOME", "/Users/test");
        std::env::set_var("WORKSPACE_PROJECTS", "/Users/test/Projects");

        let map = WorkspaceMap::from_env();
        assert_eq!(map.get("home"), Some(&"/Users/test".to_string()));
        assert_eq!(map.get("HOME"), Some(&"/Users/test".to_string())); // case insensitive
        assert_eq!(map.get("projects"), Some(&"/Users/test/Projects".to_string()));

        std::env::remove_var("WORKSPACE_HOME");
        std::env::remove_var("WORKSPACE_PROJECTS");
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
        std::env::set_var("HOME", "/Users/test");
        let map = WorkspaceMap { aliases: HashMap::new() };

        assert_eq!(resolve_workspace("~/Projects", &map).unwrap(), "/Users/test/Projects");
        assert_eq!(resolve_workspace("~", &map).unwrap(), "/Users/test");

        std::env::remove_var("HOME");
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
```

**Add to `src/lib.rs`:**
```rust
pub mod workspace;
```

---

#### 1.2 Modify `src/config.rs`

**Changes:**
1. Add `workspace_map` field to `RuntimeConfig` struct
2. Load workspace map in `RuntimeConfig::load_from_env()`

**Specific edits:**

**Line 66-103 (RuntimeConfig struct):**
```rust
/// Runtime configuration loaded from environment variables
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub database_user: String,
    pub database_pass: String,
    pub openai_api_key: Option<String>,
    pub nomic_api_key: Option<String>,
    pub tool_timeout_ms: u64,
    pub mcp_no_log: bool,
    pub log_level: String,
    pub cache_max: usize,
    pub cache_warm: usize,
    pub retrieve_candidates: usize,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub embed_strict: bool,
    pub kg_embed_entities: bool,
    pub kg_embed_observations: bool,
    pub kg_max_neighbors: usize,
    pub kg_graph_boost: f32,
    pub kg_min_edge_strength: f32,
    pub kg_timeout_ms: u64,
    pub kg_candidates: usize,
    pub verify_topk: usize,
    pub verify_min_sim: f32,
    pub verify_evidence_limit: usize,
    pub persist_verification: bool,
    // HTTP transport configuration
    pub transport: String,
    pub http_bind: std::net::SocketAddr,
    pub http_path: String,
    pub bearer_token: Option<String>,
    pub allow_token_in_url: bool,
    pub http_sse_keepalive_sec: u64,
    pub http_session_ttl_sec: u64,
    pub http_request_timeout_ms: u64,
    pub http_mcp_op_timeout_ms: Option<u64>,
    pub http_metrics_mode: String,
    // Workspace alias resolution  // ADD THIS
    pub workspace_map: crate::workspace::WorkspaceMap,  // ADD THIS
}
```

**Line 105-146 (RuntimeConfig::default()):**
```rust
impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            // ... existing fields ...
            workspace_map: crate::workspace::WorkspaceMap::from_env(),  // ADD THIS
        }
    }
}
```

**Line 346-484 (RuntimeConfig::load_from_env()):**

Add at the end before returning `cfg` (around line 481):
```rust
        // Load workspace aliases from WORKSPACE_* env vars
        cfg.workspace_map = crate::workspace::WorkspaceMap::from_env();

        cfg
```

---

### Phase 2: Tool Integration

#### 2.1 Modify `src/tools/call_cc.rs`

**Line 83-86 (cwd validation):**

**Replace:**
```rust
let cwd = normalize_optional_string(params.cwd);
let cwd = cwd.ok_or_else(|| SurrealMindError::InvalidParams {
    message: "cwd is required and cannot be empty".into(),
})?;
```

**With:**
```rust
let cwd_input = normalize_optional_string(params.cwd).ok_or_else(|| {
    SurrealMindError::InvalidParams {
        message: "cwd is required and cannot be empty".into(),
    }
})?;

// Resolve workspace alias or expand path
let cwd = crate::workspace::resolve_workspace(&cwd_input, &self.config.runtime.workspace_map)?;
```

**Note:** Assumes `SurrealMindServer` has access to `config`. If not, add:
```rust
// In src/server/mod.rs or wherever SurrealMindServer is defined
pub struct SurrealMindServer {
    // ... existing fields ...
    pub config: Arc<Config>,  // Add if not present
}
```

---

#### 2.2 Modify `src/tools/call_gem.rs`

**Line 79 (cwd handling):**

**Replace:**
```rust
let cwd = normalize_optional_string(params.cwd);
```

**With:**
```rust
let cwd_input = normalize_optional_string(params.cwd);
let cwd = if let Some(input) = cwd_input {
    Some(crate::workspace::resolve_workspace(&input, &self.config.runtime.workspace_map)?)
} else {
    None
};
```

---

#### 2.3 Modify `src/tools/call_warp.rs`

**Line 64-69 (cwd validation):**

**Replace:**
```rust
let cwd = params.cwd.trim().to_string();
if cwd.is_empty() {
    return Err(SurrealMindError::InvalidParams {
        message: "cwd is required and cannot be empty".into(),
    });
}
```

**With:**
```rust
let cwd_input = params.cwd.trim();
if cwd_input.is_empty() {
    return Err(SurrealMindError::InvalidParams {
        message: "cwd is required and cannot be empty".into(),
    });
}

// Resolve workspace alias or expand path
let cwd = crate::workspace::resolve_workspace(cwd_input, &self.config.runtime.workspace_map)?;
```

---

#### 2.4 Modify `src/tools/call_vibe.rs`

**Line 55-60 (cwd validation):**

**Replace:**
```rust
let cwd = params.cwd.trim().to_string();
if cwd.is_empty() {
    return Err(SurrealMindError::InvalidParams {
        message: "cwd is required and cannot be empty".into(),
    });
}
```

**With:**
```rust
let cwd_input = params.cwd.trim();
if cwd_input.is_empty() {
    return Err(SurrealMindError::InvalidParams {
        message: "cwd is required and cannot be empty".into(),
    });
}

// Resolve workspace alias or expand path
let cwd = crate::workspace::resolve_workspace(cwd_input, &self.config.runtime.workspace_map)?;
```

---

### Phase 3: Schema Documentation

#### 3.1 Modify `src/schemas.rs`

**Update `cwd` descriptions in all delegation tool schemas:**

**Lines 52, 91, 139, 159:**

**call_gem_schema (line 52):**
```rust
"cwd": {
    "type": "string",
    "description": "Working directory: workspace alias (e.g., 'surreal-mind', 'home') or absolute path (e.g., '/Users/sam/Projects/foo'). Use '~/' for home expansion."
},
```

**call_cc_schema (line 91):**
```rust
"cwd": {
    "type": "string",
    "description": "Working directory: workspace alias (e.g., 'surreal-mind', 'home') or absolute path (e.g., '/Users/sam/Projects/foo'). Use '~/' for home expansion."
},
```

**call_warp_schema (line 139):**
```rust
"cwd": {
    "type": "string",
    "description": "Working directory: workspace alias (e.g., 'surreal-mind', 'home') or absolute path (e.g., '/Users/sam/Projects/foo'). Use '~/' for home expansion."
},
```

**call_vibe_schema (line 159):**
```rust
"cwd": {
    "type": "string",
    "description": "Working directory: workspace alias (e.g., 'surreal-mind', 'home') or absolute path (e.g., '/Users/sam/Projects/foo'). Use '~/' for home expansion."
},
```

---

### Phase 4: Environment Configuration

#### 4.1 Update `.env.example`

**Add workspace alias examples:**

```bash
# Workspace Aliases (for delegation tool cwd parameters)
# Define shortcuts for commonly used directories
# Usage: call_cc(cwd="surreal-mind", ...) instead of full path
WORKSPACE_HOME=/Users/samuelatagana
WORKSPACE_PROJECTS=/Users/samuelatagana/Projects
WORKSPACE_SURREAL_MIND=/Users/samuelatagana/Projects/LegacyMind/surreal-mind
WORKSPACE_PHOTOGRAPHY_MIND=/Users/samuelatagana/Projects/LegacyMind/photography-mind
WORKSPACE_VAULT=/Users/samuelatagana/LegacyMind_Vault
```

---

## Test Cases

### Unit Tests (in `src/workspace.rs`)

✅ Already included in workspace.rs implementation above:
- `test_workspace_map_from_env` - Load from environment
- `test_resolve_absolute_path` - Absolute paths work
- `test_resolve_tilde_expansion` - Tilde expansion works
- `test_resolve_workspace_alias` - Alias resolution works
- `test_resolve_unknown_alias_with_suggestion` - Error messaging
- `test_levenshtein_distance` - Similarity matching

### Integration Tests

**Create `tests/workspace_resolution.rs`:**

```rust
use surreal_mind::workspace::{WorkspaceMap, resolve_workspace};
use std::collections::HashMap;

#[test]
fn test_call_cc_with_workspace_alias() {
    // Set up environment
    std::env::set_var("WORKSPACE_TEST", "/tmp/test");

    let map = WorkspaceMap::from_env();
    let resolved = resolve_workspace("test", &map).unwrap();

    assert_eq!(resolved, "/tmp/test");

    std::env::remove_var("WORKSPACE_TEST");
}

#[test]
fn test_backward_compatibility_absolute_paths() {
    let map = WorkspaceMap::from_env();

    // Absolute paths should still work
    let resolved = resolve_workspace("/var/local/foo", &map).unwrap();
    assert_eq!(resolved, "/var/local/foo");
}

#[test]
fn test_error_suggests_valid_aliases() {
    std::env::set_var("WORKSPACE_SURREAL_MIND", "/path/to/surreal-mind");

    let map = WorkspaceMap::from_env();
    let result = resolve_workspace("surreal", &map);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("surreal-mind"));
    assert!(err_msg.contains("Did you mean"));

    std::env::remove_var("WORKSPACE_SURREAL_MIND");
}
```

**Run tests:**
```bash
cargo test --all
cargo test workspace
```

---

## Implementation Order

Follow this sequence to avoid dependency issues:

1. ✅ **Create `src/workspace.rs`** - No dependencies
2. ✅ **Add `pub mod workspace;` to `src/lib.rs`**
3. ✅ **Modify `src/config.rs`** - Add workspace_map field and loading
4. ✅ **Verify compilation:** `cargo check`
5. ✅ **Run unit tests:** `cargo test workspace`
6. ✅ **Modify delegation tools** (call_cc, call_gem, call_warp, call_vibe)
7. ✅ **Update schemas.rs** - Documentation only
8. ✅ **Update .env.example** - Documentation only
9. ✅ **Create integration tests**
10. ✅ **Full test suite:** `cargo test --all`
11. ✅ **Manual testing** - Start server with workspace env vars, test delegation

---

## Acceptance Criteria

**Must Have:**
- [ ] Workspace aliases resolve correctly (e.g., "surreal-mind" → full path)
- [ ] Absolute paths still work unchanged (`/absolute/path`)
- [ ] Tilde expansion works (`~/Projects/foo`)
- [ ] Unknown aliases return helpful error with suggestions
- [ ] Error messages list available workspaces
- [ ] All 4 delegation tools support workspace resolution
- [ ] All existing tests pass (`cargo test --all`)
- [ ] New unit tests pass (workspace.rs)
- [ ] Documentation updated (schemas, .env.example)

**Should Have:**
- [ ] Integration tests for each tool
- [ ] Levenshtein distance suggestions work (<4 edit distance)
- [ ] Case-insensitive alias matching (HOME = home)
- [ ] Empty workspace map handled gracefully

**Nice to Have:**
- [ ] CHANGELOG.md entry
- [ ] Update CLAUDE.md with workspace usage examples
- [ ] Update tool documentation in docs/AGENTS/tools.md

---

## Manual Testing Procedure

1. **Set up environment:**
   ```bash
   export WORKSPACE_HOME=/Users/samuelatagana
   export WORKSPACE_SURREAL_MIND=/Users/samuelatagana/Projects/LegacyMind/surreal-mind
   export WORKSPACE_TEST=/tmp/test_workspace
   ```

2. **Restart surreal-mind server:**
   ```bash
   launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind
   ```

3. **Test cases via MCP:**
   ```python
   # Valid alias
   call_cc(cwd="surreal-mind", prompt="echo hello")
   # Expected: Works, resolves to full path

   # Invalid alias with suggestion
   call_cc(cwd="surreal", prompt="echo hello")
   # Expected: Error suggests "surreal-mind"

   # Absolute path (backward compat)
   call_cc(cwd="/tmp", prompt="echo hello")
   # Expected: Works unchanged

   # Tilde expansion
   call_cc(cwd="~/Projects", prompt="echo hello")
   # Expected: Expands to /Users/samuelatagana/Projects

   # Empty cwd
   call_cc(cwd="", prompt="echo hello")
   # Expected: Error "cwd cannot be empty"
   ```

4. **Test all 4 tools:**
   - `call_cc`
   - `call_gem`
   - `call_warp`
   - `call_vibe`

---

## Rollback Plan

If issues arise:
1. **Git revert** - All changes are in discrete commits
2. **Remove workspace_map from RuntimeConfig** - Falls back to raw path behavior
3. **Env vars are optional** - If no WORKSPACE_* vars set, old behavior continues

---

## Notes

- **No breaking changes** - All existing code/scripts using absolute paths continue working
- **Opt-in feature** - Only active if WORKSPACE_* env vars are set
- **Performance impact** - Negligible (O(1) HashMap lookup per tool call)
- **Memory overhead** - ~100 bytes per alias (typically <10 aliases)
- **Security** - No new attack surface (env vars already trusted)

---

## Future Enhancements (Out of Scope)

- Dynamic workspace reloading (SIGHUP handler)
- Per-user workspace configs (~/.surreal-mind/workspaces.toml)
- Workspace validation (check if path exists on load)
- Shell completion for workspace names
- `list_workspaces` MCP tool
