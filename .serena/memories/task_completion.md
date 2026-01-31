# Task Completion Checklist

Before considering a task complete:
1.  **Format Code:** Run `cargo fmt --all`.
2.  **Lint Check:** Run `cargo clippy --workspace --all-targets -- -D warnings`.
3.  **Run Tests:** Run `cargo test --workspace`.
4.  **Verify MCP:** If tool schemas changed, run `cargo test --test tool_schemas`.
5.  **Smoke Test:** Run `./tests/test_mcp_comprehensive.sh`.
6.  **Update CHANGELOG:** Add a summary of changes to `CHANGELOG.md`.
7.  **Memory Check:** Update `GEMINI.md` if any architectural or strategic decisions were made.
