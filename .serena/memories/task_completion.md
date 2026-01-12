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
Address any warnings. New code should be clean.

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
2. Update `src/server/router.rs` if tool name/routing changed
3. Update `tests/tool_schemas.rs` if tool roster changed
4. Update Serena memories (this file, project_overview, code_structure)

## After Making Schema Changes

If you modified `src/server/schema.rs`:
1. Test with fresh database or run migration
2. Check that indexes are created properly
3. Update relevant documentation

## Testing Protocol

When asked to test:
1. Run the requested tests (no setup, no troubleshooting)
2. Capture results as-is
3. Deliver to requester or append to referenced testing document
4. Do not investigate failures - that's a separate task

## Documentation Updates

For significant changes:
1. Update `CLAUDE.md` if architecture/commands change
2. Update Serena memories to stay current
3. Update `CHANGELOG.md` if releasing
