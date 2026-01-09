# Task 40: Remove Scalpel Tool and Local Delegation Infrastructure

## Status: COMPLETED

Linked doc: `backlog/docs/doc-4 - Implementation-Steps-remove-scalpel-tool.md`

## Objective
Remove the `scalpel` tool and all associated infrastructure from surreal-mind. The scalpel tool was a local model delegation feature using mistralrs-server that isn't performing reliably on current hardware (32GB Studio). Local delegation is being deprioritized in favor of remote delegation tools (call_gem) that provide better reliability and performance.

---

## Background Context

**What is Scalpel?**
- Local delegation tool using mistralrs-server (Hermes-3-Llama-3.2-3B model)
- Designed for fast, local "routine operations" without cloud dependencies
- Two modes: `intel` (read-only) and `edit` (full access)
- Fire-and-forget background execution pattern
- Tool access via JSON protocol

**Why Remove It?**
- Performance unreliable on 32GB Studio hardware
- Tool-use protocol issues (documented in task-39)
- Remote delegation (call_gem) provides better reliability
- Reduces codebase complexity and maintenance burden
- Frees up port 8111 and reduces background service count

**Historical Context:**
- Added: Task-30 (Add scalpel local delegation tool)
- Model swaps: Task-32 (Ministral), Task-37 (Qwen), Task-38 (Hermes-3)
- Related tasks: Task-35 (KG prompt tuning), Task-36 (fire-and-forget modes)

---

## Files to Remove

### Primary Implementation Files
1. **`src/tools/scalpel.rs`** - Main scalpel tool implementation (~500+ lines)
2. **`src/server/scalpel_helpers.rs`** - HTTP helpers for scalpel operations
3. **`tests/test_scalpel_operations.rs`** - Scalpel-specific tests
4. **`scripts/start_scalpel_server.sh`** - Server launch script

### Backlog/Documentation (Move to Archive)
5. **`backlog/completed/task-30 - Add-scalpel-local-delegation-tool.md`**
6. **`backlog/completed/task-32 - Run-Ministral-3B-via-mistralrs.md`**
7. **`backlog/completed/task-34 - Fix-call_cancel-to-actually-stop-running-delegate-jobs.md`** (partial)
8. **`backlog/completed/task-37 - Swap-Scalpel-to-Qwen2.5.md`**
9. **`backlog/completed/task-38 - Swap-Scalpel-to-Hermes3.md`**
10. **`backlog/tasks/task-35 - Scalpel-KG-prompt-tuning.md`** (blocked task)
11. **`backlog/tasks/task-36 - Scalpel-fire-and-forget-modes.md`** (blocked task)
12. **`backlog/active/task-39 - Fix-Hermes-3-tool-use-protocol.md`** (obsolete)

---

## Code References to Clean Up

### Source Files with Scalpel References (verify and update)
1. **`src/registry.rs`** - Remove scalpel from tool registry
2. **`src/schemas.rs`** - Remove scalpel tool schema definitions
3. **`src/tools/mod.rs`** - Remove scalpel module import
4. **`src/server/mod.rs`** - Remove scalpel handler references
5. **`src/server/router.rs`** - Remove scalpel route definitions

### Search for Additional References
Run comprehensive search before finalizing:
```bash
grep -r "scalpel" --include="*.rs" --include="*.toml" --include="*.md" . \
  --exclude-dir=target --exclude-dir=.git
```

Expected ~75 references to audit.

---

## Implementation Plan

### Phase 1: Backup and Preparation (10 minutes)
1. Create backup branch: `git checkout -b backup/remove-scalpel`
2. Document current scalpel configuration in task notes
3. Verify no active dependencies on scalpel in production workflows
4. Check for any environment-specific scalpel configurations

### Phase 2: Remove Primary Files (20 minutes)
1. Delete implementation files:
   ```bash
   rm src/tools/scalpel.rs
   rm src/server/scalpel_helpers.rs
   rm tests/test_scalpel_operations.rs
   rm scripts/start_scalpel_server.sh
   ```
2. Remove scalpel module from `src/tools/mod.rs`
3. Remove scalpel helpers from `src/server/mod.rs`

### Phase 3: Clean Up Registry and Schemas (15 minutes)
1. Remove scalpel tool registration from `src/registry.rs`
2. Remove scalpel schema definitions from `src/schemas.rs`
3. Remove scalpel routes from `src/server/router.rs`
4. Verify no dangling imports or references

### Phase 4: Archive Related Tasks (10 minutes)
1. Move completed scalpel tasks to `backlog/archive/scalpel/`:
   - task-30, task-32, task-34 (partial), task-37, task-38
2. Close/archive blocked tasks (task-35, task-36)
3. Mark task-39 as obsolete (scalpel removed)
4. Update backlog/docs if they reference scalpel

### Phase 5: Validation (20 minutes)
1. **Compile check**: `cargo check --workspace --all-targets`
2. **Clippy validation**: `cargo clippy --workspace --all-targets -- -D warnings`
3. **Test suite**: `cargo test --workspace --all-features`
4. **Format**: `cargo fmt --all`
5. Verify no scalpel references remain: `grep -r "scalpel" src/`

### Phase 6: Documentation Updates (15 minutes)
1. **README.md**: Remove scalpel from tool list (if present)
2. **CHANGELOG.md**: Add removal entry with rationale
3. **docs/AGENTS/tools.md**: Remove scalpel tool documentation
4. **docs/AGENTS/maintenance.md**: Remove scalpel server startup instructions
5. Update this task document with completion notes

### Phase 7: Commit and Verify (10 minutes)
1. Review all changes with `git status` and `git diff`
2. Commit with atomic message:
   ```
   Remove scalpel tool and local delegation infrastructure

   - Delete scalpel implementation (src/tools/scalpel.rs)
   - Remove scalpel server helpers and routes
   - Clean up registry, schemas, and test files
   - Archive related task documents
   - Update CHANGELOG and documentation

   Rationale: Local delegation unreliable on 32GB hardware.
   Remote delegation (call_gem) provides better reliability.

   Co-Authored-By: rust-builder <noreply@legacymind.ai>
   ```
3. Run final smoke test of remaining tools
4. Push to remote: `git push origin main`

---

## Testing Plan

### Compilation Tests
```bash
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-features
cargo build --release
```

### Functionality Tests
Verify remaining delegation tools work:
```bash
# Test call_gem (primary delegation tool)
# - Simple prompt test
# - Background fire-and-forget test
# - call_status verification
# - call_jobs listing
# - call_cancel functionality
```

### Negative Tests
Verify scalpel is completely removed:
```bash
# No scalpel references in source
grep -r "scalpel" src/ && echo "FAIL: References remain" || echo "PASS"

# No scalpel schema in MCP tools list
# (test via MCP client tool listing)
```

---

## Acceptance Criteria

- [ ] All scalpel implementation files deleted
- [ ] All scalpel references removed from source code
- [ ] Registry, schemas, and router cleaned up
- [ ] Tests compile and pass (`cargo test --workspace`)
- [ ] Clippy passes with zero warnings (`-D warnings`)
- [ ] Format clean (`cargo fmt --all`)
- [ ] Documentation updated (README, CHANGELOG, AGENTS docs)
- [ ] Related task files archived appropriately
- [ ] No scalpel references in final codebase search
- [ ] Remaining delegation tools (call_gem) still functional
- [ ] Git commit follows project standards
- [ ] Changes pushed to remote repository

---

## Success Metrics

**Before Removal:**
- Tool count: 10 (including scalpel)
- Scalpel code: ~500+ lines
- Active scalpel tasks: 3 (task-35, task-36, task-39)
- grep "scalpel" count: ~75 references

**After Removal (Target):**
- Tool count: 9 (scalpel removed)
- Scalpel code: 0 lines
- Active scalpel tasks: 0 (all archived/obsolete)
- grep "scalpel" count: 0 references
- Compile time: Potentially faster (less code)
- Maintenance burden: Reduced

---

## Dependencies

**Rust Validation Tools:**
- cargo check, clippy, test, fmt (standard toolchain)

**Files Modified:**
- `src/registry.rs` (remove registration)
- `src/schemas.rs` (remove schema)
- `src/tools/mod.rs` (remove module)
- `src/server/mod.rs` (remove handlers)
- `src/server/router.rs` (remove routes)
- `README.md` (update tool list)
- `CHANGELOG.md` (add removal entry)
- `docs/AGENTS/tools.md` (remove scalpel docs)
- `docs/AGENTS/maintenance.md` (remove server instructions)

**Files Deleted:**
- 4 primary files (scalpel.rs, scalpel_helpers.rs, test, script)

**Files Archived:**
- 9 task documents (moved to backlog/archive/scalpel/)

---

## Risk Assessment

**Low Risk:**
- Scalpel is isolated feature (no deep dependencies)
- Removal doesn't affect core consciousness/KG functionality
- Other delegation tools (call_gem) remain functional
- Compilation errors will surface immediately

**Medium Risk:**
- Potential for missed references in comments/docs
- Archive task documents might be needed for historical reference

**Mitigation:**
- Comprehensive grep search before/after removal
- Keep backup branch for 30 days
- Archive task documents (don't delete them)
- Thorough testing with cargo check/clippy/test
- Document rationale clearly in CHANGELOG

---

## Rollback Plan

If removal causes unexpected issues:

1. **Immediate Rollback:**
   ```bash
   git checkout backup/remove-scalpel
   git checkout -b main-rollback
   git branch -D main
   git checkout -b main
   ```

2. **Selective Restoration:**
   - Cherry-pick specific scalpel files if needed
   - Restore from git history: `git show <commit>:path/to/file > file`

3. **Forward Fix:**
   - If specific functionality needed, implement minimal version
   - Consider alternative delegation approaches (call_gem enhancement)

---

## Related Tasks

**Completed (Being Archived):**
- Task-30: Add scalpel local delegation tool
- Task-32: Run Ministral 3B via mistralrs
- Task-34: Fix call_cancel for delegate jobs
- Task-37: Swap Scalpel to Qwen2.5
- Task-38: Swap Scalpel to Hermes3

**Blocked/Obsolete (Being Closed):**
- Task-35: Scalpel KG prompt tuning
- Task-36: Scalpel fire-and-forget modes
- Task-39: Fix Hermes-3 tool-use protocol

**Unaffected (Remain Active):**
- Call_gem delegation system (primary delegation tool)
- Core thinking/search/remember functionality
- Knowledge graph operations

---

## Notes

**Key Insight**: Local delegation was aspirational but hardware-constrained. The 32GB Studio can't reliably run mistralrs-server alongside SurrealDB and other services. Remote delegation via call_gem provides better reliability and performance, accepting the tradeoff of cloud dependency for critical operations.

**Alternative Considered**: Fix scalpel tool-use protocol (task-39). Rejected because even if fixed, hardware constraints remain and maintenance burden isn't justified when call_gem works reliably.

**Future Considerations**: If local delegation becomes critical again:
- Consider lighter models (sub-1B parameters)
- Wait for Mac Studio upgrade (more RAM)
- Evaluate alternative local inference servers (llama.cpp, ollama)
- But don't implement until proven necessary

**Post-Removal Cleanup**: After 30 days, if no issues:
- Delete backup branch
- Archive this task to completed/
- Consider removing archived scalpel tasks from backlog/

---

## Completion Checklist

When marking this task complete, verify:
- [ ] All acceptance criteria met
- [ ] CHANGELOG.md updated with removal entry
- [ ] Documentation reflects current tool surface (9 tools)
- [ ] No regression in existing functionality
- [ ] Clean git history (atomic commit, clear message)
- [ ] Task-39 marked as obsolete
- [ ] Related tasks archived properly
- [ ] Final grep confirms zero scalpel references

## Codex Review

- Schema/backward-compat: If `ToolName`/`ToolKind` enums (and any serde tags in schemas.rs) lose the `Scalpel` variant, deserialization of existing SurrealDB rows or archived JSON logs containing that tag will panic/fail. Consider keeping a `DeprecatedScalpel`/`#[serde(other)]` catch-all or run a DB migration to rewrite stored values before removing the variant.
- Feature flags/deps: Check `Cargo.toml` for scalpel-specific features (e.g., `scalpel` feature gate, mistralrs client deps, http client config). Removing the code without pruning the feature and dependencies leaves unused crates and could keep transitive features enabled (build time/attack surface). Run `cargo tree -i mistralrs-server` after edits to confirm removal.
- Router/registry fallthrough: When deleting routes/handlers, ensure `router.rs` doesn’t leave an empty `match` arm or unreachable branch that causes a compile error only under specific `cfg` (e.g., `feature = "server"` vs `tooling-only`). Audit `#[cfg(feature = "server")]` blocks that previously gated scalpel.
- Tool discovery contract: If the MCP tool list is enumerated and consumed by clients, make sure clients handle the missing tool gracefully (no hard-coded ordinal indexes). If there is an OpenAPI/JSON schema export step, regenerate it so consumers don’t see stale scalpel entries.
- Metrics/logging: Remove or redirect any `tracing` metrics keyed on `scalpel_*` to avoid orphaned Prometheus counters and to keep dashboards from breaking on missing series.
- Tests relying on shared fixtures: Some integration/unit tests may import shared fixtures that include a scalpel entry (e.g., common tool registry fixtures). Prune those fixtures rather than only deleting the scalpel-specific test file to avoid hidden compile/test failures.
- Port release validation: After removing `start_scalpel_server.sh`, confirm no other services still bind to 8111 in dev scripts or launchd plists; update any health-check lists that assumed that port.
- Build scripts: If `build.rs` or generated schema files embed tool lists, rerun the generation step (or remove the generation hook) so scalpel doesn’t linger in generated code.


## Gemini Review

Suggestions and refinements for the Scalpel removal plan:

### 1. Additional File Removal
The plan should include these files which are exclusively used by Scalpel:
- **`src/clients/local.rs`**: Contains the `LocalClient` for mistralrs-server. Dead code once Scalpel is removed.
- **`tests/test_append_behavior.rs`**: Specifically tests Scalpel-style file append logic.

### 2. Registry & Module Cleanup
- **`src/clients/mod.rs`**: Remove `pub mod local;`.
- **`src/registry.rs`**: Update the doc comment to remove the `call_scalpel` reference.
- **`.env.example`**: Prune all `SURR_SCALPEL_*` variables.
- **`src/schemas.rs`**: Remove `scalpel_schema()` (already in plan) and ensure no other schemas reference it.

### 3. Database & State Consistency
- **Stale Job Records**: Existing records in `agent_jobs` with `tool_name = 'scalpel'` might cause deserialization issues or UI noise in `call_jobs`. Recommend adding a step to clear these: `DELETE agent_jobs WHERE tool_name = 'scalpel'`.
- **Origin Metadata**: Thoughts with `origin = 'scalpel'` will remain in the DB. This is fine for history, but should be noted if a fully "clean" state is required.

### 4. Backlog Maintenance
- **Ghost Files**: Delete/archive the percent-encoded duplicate: `backlog/tasks/task-35%20-%20Scalpel-KG-prompt-tuning.md`.

### 5. Client/System Prompts
- Ensure that any external system instructions (MCP configuration or main agent system prompts) are updated to stop advertising `scalpel`. If the model thinks the tool exists, it will trigger "Method not found" errors.

### 6. Implementation Sequence
- Pruning `src/server/router.rs` and `src/registry.rs` *before* deleting implementation files ensures the project remains compilable during each step of the removal.

## Vibe Review

### 1. Comprehensive Testing Strategy
The current testing plan focuses on compilation and basic functionality. Consider adding:
- **Integration tests**: Verify that call_gem works correctly in scenarios where scalpel was previously used
- **Performance tests**: Measure if the removal improves overall system performance
- **Memory usage tests**: Verify reduced memory footprint after removal

### 2. Documentation Updates
The plan mentions updating README.md, CHANGELOG.md, and AGENTS docs. Also consider:
- **API documentation**: Update any OpenAPI/Swagger docs that might reference scalpel
- **User guides**: Ensure user-facing documentation doesn't mention scalpel
- **Troubleshooting guides**: Remove any scalpel-specific troubleshooting sections

### 3. Monitoring and Observability
- **Metrics cleanup**: Remove any scalpel-specific metrics from monitoring dashboards
- **Logging**: Update logging configuration to remove scalpel-related log entries
- **Alerts**: Remove any alerts that might be triggered by scalpel operations

### 4. Security Considerations
- **Port security**: Verify that port 8111 is no longer exposed or referenced in any security configurations
- **Dependency removal**: Ensure all scalpel-related dependencies are removed from Cargo.toml to reduce attack surface
- **Secret management**: Verify that no scalpel-related secrets remain in configuration files

### 5. User Communication
- **Deprecation notice**: Consider adding a deprecation notice period before complete removal
- **Migration guide**: Provide guidance for users who were using scalpel on how to transition to call_gem
- **Release notes**: Clearly document the removal in release notes with rationale

### 6. Continuous Integration
- **CI pipeline updates**: Update any CI/CD pipelines that might reference scalpel
- **Test coverage**: Ensure test coverage remains adequate after removal
- **Build validation**: Add validation steps to prevent scalpel references from being reintroduced

### 7. Performance Optimization
- **Code size reduction**: Measure and document the reduction in code size
- **Build time improvement**: Measure if build times improve after removal
- **Runtime performance**: Verify if runtime performance improves with reduced complexity

### 8. Future-Proofing
- **Modular design**: Consider if the removal provides an opportunity to improve the overall modularity of the codebase
- **Documentation of lessons learned**: Document why scalpel was removed and what was learned for future tool development
- **Architecture review**: Use this as an opportunity to review the overall architecture and identify other potential simplifications

## Vibe Review

### 1. Comprehensive Testing Strategy
The current testing plan focuses on compilation and basic functionality. Consider adding:
- **Integration tests**: Verify that call_gem works correctly in scenarios where scalpel was previously used
- **Performance tests**: Measure if the removal improves overall system performance
- **Memory usage tests**: Verify reduced memory footprint after removal

### 2. Documentation Updates
The plan mentions updating README.md, CHANGELOG.md, and AGENTS docs. Also consider:
- **API documentation**: Update any OpenAPI/Swagger docs that might reference scalpel
- **User guides**: Ensure user-facing documentation doesn't mention scalpel
- **Troubleshooting guides**: Remove any scalpel-specific troubleshooting sections

### 3. Monitoring and Observability
- **Metrics cleanup**: Remove any scalpel-specific metrics from monitoring dashboards
- **Logging**: Update logging configuration to remove scalpel-related log entries
- **Alerts**: Remove any alerts that might be triggered by scalpel operations

### 4. Security Considerations
- **Port security**: Verify that port 8111 is no longer exposed or referenced in any security configurations
- **Dependency removal**: Ensure all scalpel-related dependencies are removed from Cargo.toml to reduce attack surface
- **Secret management**: Verify that no scalpel-related secrets remain in configuration files

### 5. User Communication
- **Deprecation notice**: Consider adding a deprecation notice period before complete removal
- **Migration guide**: Provide guidance for users who were using scalpel on how to transition to call_gem
- **Release notes**: Clearly document the removal in release notes with rationale

### 6. Continuous Integration
- **CI pipeline updates**: Update any CI/CD pipelines that might reference scalpel
- **Test coverage**: Ensure test coverage remains adequate after removal
- **Build validation**: Add validation steps to prevent scalpel references from being reintroduced

### 7. Performance Optimization
- **Code size reduction**: Measure and document the reduction in code size
- **Build time improvement**: Measure if build times improve after removal
- **Runtime performance**: Verify if runtime performance improves with reduced complexity

### 8. Future-Proofing
- **Modular design**: Consider if the removal provides an opportunity to improve the overall modularity of the codebase
- **Documentation of lessons learned**: Document why scalpel was removed and what was learned for future tool development
- **Architecture review**: Use this as an opportunity to review the overall architecture and identify other potential simplifications
