# SurrealMind Stdio Persistence Bug & Workaround

## Discovery Date
2025-09-14 CDT

## Issue
The rmcp stdio transport doesn't persist data to SurrealDB, while HTTP transport works perfectly.

## Root Cause
The issue appears to be in rmcp's stdio transport layer (versions 0.6.3 and potentially 0.6.4). The stdio implementation is minimal - just returns `tokio::io::stdin()` and `tokio::io::stdout()` without explicit flushing or proper async completion handling.

## Evidence
- Same `Arc<Surreal<Client>>` database connection used for both transports
- Same CREATE query with proper `.await?`
- HTTP writes instantly succeed and persist
- Stdio returns success UUIDs but data never reaches database
- Code location: `/src/main.rs` lines 134 (HTTP) vs 149 (stdio)

## Workaround

### Option 1: Use HTTP Transport (Recommended)
Configure your MCP client to use HTTP instead of stdio:

```json
{
  "mcpServers": {
    "surreal-mind": {
      "command": "/path/to/surreal-mind",
      "args": [],
      "env": {
        "SURR_TRANSPORT": "http",
        "SURR_HTTP_PORT": "3030"
      }
    }
  }
}
```

Then connect to: `http://localhost:3030`

### Option 2: Upgrade to rmcp 0.6.4+
We've upgraded from rmcp 0.6.3 to 0.6.4, which includes:
- New `title` and `icons` fields for Tool struct
- New `title`, `website_url`, and `icons` fields for Implementation struct
- Potential stdio fixes (testing required)

### Breaking Changes in rmcp 0.6.4
When upgrading, you must add these fields to all Tool initializations:
```rust
Tool {
    name: "tool_name".into(),
    title: Some("Tool Title".into()),  // NEW
    icons: None,                        // NEW
    // ... rest of fields
}
```

And to Implementation:
```rust
Implementation {
    name: "surreal-mind".to_string(),
    title: Some("Surreal Mind".to_string()),      // NEW
    version: "0.1.0".to_string(),
    website_url: Some("https://...".to_string()),  // NEW
    icons: None,                                   // NEW
}
```

## Test Script
Use `test_stdio_persistence.sh` to verify if stdio is working after upgrades.

## Status
- ✅ rmcp 0.6.4 upgrade complete
- ✅ Breaking changes fixed
- ✅ HTTP transport confirmed working
- ⏳ Stdio fix verification pending

## Impact
This bug affects ALL MCP servers using rmcp with stdio transport. Consider filing an issue with the rmcp project if 0.6.4 doesn't fix it.