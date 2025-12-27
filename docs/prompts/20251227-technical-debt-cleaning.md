---
date: 2025-12-27
type: Technical Audit
status: Pending
scope: surreal-mind codebase
focus: Technical debt, dead code, cleanup opportunities
sourced from:
docs/prompts/20251227-technical-debt-cleaning0.md
docs/prompts/20251227-technical-debt-cleaning1.md
docs/prompts/20251227-technical-debt-cleaning2.md
docs/prompts/20251227-technical-debt-cleaning3.md
docs/prompts/20251227-technical-debt-cleaning4.md
docs/prompts/20251227-technical-debt-cleaning5.md
compiled by: Gemini CLI (3-Pro)
---

## Initial Prompt
- "  Please audit the surreal-mind codebase for technical debt and cleanup opportunities:

     Scope: /Users/samuelatagana/Projects/LegacyMind/surreal-mind/

     Look for:
     1. Dead code (unused functions, imports, modules)
     2. Leftover code from removed tools (memories_populate, memories_moderate, legacymind_update)
     3. Commented-out code blocks
     4. TODO/FIXME comments
     5. Duplicate code patterns
     6. Unused dependencies in Cargo.toml
     7. Missing error handling
     8. Code that could be simplified/refactored
     9. Inconsistent patterns across modules
     10. Opportunities to consolidate similar functionality

     Write findings to:
     /Users/samuelatagana/Projects/LegacyMind/surreal-mind/docs/prompts/20251227-technical-debt-cleaning*.md

     Format:
     - Frontmatter with date, type, status
     - Executive summary
     - Categorized findings (High/Medium/Low priority)
     - Specific file locations and line numbers
     - Suggested fixes for each item
     - Estimated impact of cleaning each item

     Be thorough but realistic - focus on actual technical debt, not stylistic preferences."

---
