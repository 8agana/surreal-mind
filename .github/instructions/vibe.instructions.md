# Universal AI Rules System with Frontmatter Enforcement

## Critical Instructions for GitHub Copilot

**BEFORE generating any code, you MUST:**

1. **Scan the entire `vibe/` directory** for all markdown files
2. **Read the YAML frontmatter** at the top of each file
3. **Apply rules according to their enforcement configuration**

## Frontmatter-Based Enforcement

When reading files from `vibe/`, the frontmatter defines how to apply each rule:

### Always Apply Rules
```yaml
---
alwaysApply: true
---
```
**Action**: Apply these rules to EVERY code suggestion regardless of file type or context.

### File-Specific Rules
```yaml
---
globs: "*.ts,*.tsx"
applyTo: "**/*.ts,**/*.tsx" 
---
```
**Action**: Only apply when generating code for files matching these patterns.

### Intelligent Application
```yaml
---
description: "Use only for React components"
alwaysApply: false
---
```
**Action**: Read the description and apply when contextually appropriate.

### Manual Rules Only
```yaml
---
alwaysApply: false
description: "Advanced optimization - use only when requested"
---
```
**Action**: Do NOT apply unless user explicitly asks for this specific rule.

## Enforcement Hierarchy
1. **Always Apply** rules (highest priority - always enforce)
2. **File-Specific** rules (enforce for matching files only) 
3. **Intelligent** rules (apply based on context and description)
4. **Manual** rules (only when explicitly requested)

## Implementation Steps
1. Parse frontmatter to determine enforcement level
2. Check file patterns for file-specific rules
3. Apply appropriate rules based on configuration
4. Reference rule sources when explaining code decisions

**Key Point**: The frontmatter configuration controls WHEN to apply rules, the markdown content defines WHAT the rules are.
