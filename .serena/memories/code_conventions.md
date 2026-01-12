# Code Conventions for surreal-mind

## Rust Style

### Edition
Rust 2024 edition (cutting edge)

### Formatting
- `cargo fmt` enforced
- Standard rustfmt defaults

### Error Handling
- Custom `SurrealMindError` enum in `src/error.rs`
- Variants: `Mcp`, `InvalidParams`, `Database`, `Embedding`, `Serialization`, `Internal`, `Timeout`
- Use `Result<T>` alias from `src/error.rs`

### Async
- Tokio runtime (full features)
- `async fn` for all I/O operations
- `#[async_trait]` for trait async methods

## Tool Parameters

### Struct Pattern
```rust
#[derive(Debug, Deserialize)]
pub struct MyToolParams {
    pub required_field: String,
    #[serde(default)]
    pub optional_field: Option<String>,
}
```

### Normalization
Use `normalize_optional_string()` helper to convert empty strings to None.

## Client Pattern

### Builder Style
```rust
let client = GeminiClient::new()
    .with_timeout_ms(120_000)
    .with_cwd("/some/path");
```

### Trait Implementation
Implement `CognitiveAgent` trait for CLI wrappers:
```rust
#[async_trait]
impl CognitiveAgent for MyClient {
    async fn call(&self, prompt: &str, session_id: Option<&str>) 
        -> Result<AgentResponse, AgentError>;
}
```

## Naming

- Tool handlers: `handle_<tool_name>`
- Schema functions: `<tool_name>_schema`
- Params structs: `<ToolName>Params`
- Files match tool names: `delegate_gemini.rs` for `call_gem` tool

## Testing

- Unit tests in source files (`#[cfg(test)]` modules)
- Integration tests in `tests/`
- Key test file: `tests/tool_schemas.rs` validates all tool schemas
- Run `cargo test --all` before committing
