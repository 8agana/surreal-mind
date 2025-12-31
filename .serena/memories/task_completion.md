# Task Completion Checklist for surreal-mind

## Before Committing Changes

### 1. Build Check
```bash
cargo build
```
Must compile without errors.

### 2. Format
```bash
cargo fmt --all
```
Run formatter to ensure consistent style.

### 3. Lint
```bash
cargo clippy
```
Address any warnings. Pre-existing warnings may be acceptable but new code should be clean.

### 4. Tests
```bash
cargo test --all
```
All tests must pass. Key test file: `tests/tool_schemas.rs` for tool definitions.

### 5. Full CI Check (Optional)
```bash
make ci
```
Runs: check → fmt-check → lint → test

## After Making Tool Changes

If you modified a tool:
1. Update schema in `src/schemas.rs` if parameters changed
2. Update tool count in `README.md` if tools added/removed
3. Update `tests/tool_schemas.rs` if tool roster changed

## Documentation Updates

For significant changes:
1. Update `docs/prompts/` with implementation notes
2. Update `CHANGELOG.md` if releasing

## Code Style Notes

- Rust 2024 edition
- Use `#[serde(default)]` for optional params
- Prefer `Result<T>` with custom `SurrealMindError`
- Builder pattern for clients (e.g., `GeminiClient::new().with_cwd(...)`)
