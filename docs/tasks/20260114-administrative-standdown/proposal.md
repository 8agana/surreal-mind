# Administrative Standdown - SurrealMind Consolidation

**Created:** 2026-01-14
**Status:** Planning
**Duration:** ~1 month
**Goal:** Ensure every document, piece of code, and script serves a known purpose

---

## Overview

After months of rapid feature development (phases 1-9), SurrealMind needs consolidation. No new features - just making what exists solid, consistent, and well-documented.

This is not about adding capabilities. It's about:
- Understanding exactly what we have
- Removing what doesn't belong
- Fixing inconsistencies
- Ensuring test coverage
- Cleaning the knowledge graph

---

## Phases

### Phase 1: Comprehensive Audit (Gemini)
**Owner:** Gemini (1M context window)
**Tracking:** [audit-findings.md](audit-findings.md)

Load the entire codebase and produce:
1. **Inventory** - every file, purpose, dependencies
2. **Orphans** - dead code, stale docs, duplicate scripts
3. **Inconsistencies** - naming, error handling, patterns
4. **Questions** - things that need human clarification

### Phase 2: Code Cleanup
**Owner:** CC + smcc
**Tracking:** [code-cleanup.md](code-cleanup.md)

Work through audit findings:
- Remove orphaned code
- Standardize patterns
- Fix identified inconsistencies
- Add missing error handling

### Phase 3: Documentation Cleanup
**Owner:** CC
**Tracking:** [docs-cleanup.md](docs-cleanup.md)

- Update stale documentation
- Remove docs for deleted features
- Ensure README, CHANGELOG, AGENTS.md are current
- Verify Serena memories match reality

### Phase 4: Test Coverage
**Owner:** CC + smcc
**Tracking:** [test-coverage.md](test-coverage.md)

- Audit existing test coverage
- Add tests for uncovered edge cases
- Ensure all MCP tools have verification tests
- Document test procedures

### Phase 5: Knowledge Graph Audit
**Owner:** CC
**Tracking:** [kg-audit.md](kg-audit.md)

- Review entity quality
- Prune stale/redundant observations
- Verify relationship integrity
- Clean up experimental data from early development

---

## Roles

| Role | Responsibility |
|------|----------------|
| **Gemini** | Initial comprehensive audit (Phase 1) |
| **CC (DT)** | Coordination, doc cleanup, KG audit |
| **smcc** | Code changes, testing in surreal-mind directory |
| **Sam** | Decision authority on what stays/goes |

---

## Success Criteria

- [ ] Every file has documented purpose
- [ ] No orphaned code paths
- [ ] Consistent error handling patterns
- [ ] All MCP tools have tests
- [ ] Documentation matches implementation
- [ ] KG contains only intentional, useful data

---

## Linked Documents

- [Gemini Audit Prompt](gemini-audit-prompt.md)
- [Audit Findings](audit-findings.md)
- [Code Cleanup Tracking](code-cleanup.md)
- [Documentation Cleanup Tracking](docs-cleanup.md)
- [Test Coverage Tracking](test-coverage.md)
- [KG Audit Tracking](kg-audit.md)
