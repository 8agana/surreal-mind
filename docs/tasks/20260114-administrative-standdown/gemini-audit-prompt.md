# Gemini Comprehensive Audit Prompt

**Purpose:** Load the entire surreal-mind codebase and produce a systematic audit for the Administrative Standdown initiative.

---

## The Prompt

```
You are conducting a comprehensive audit of the surreal-mind project - a Rust-based MCP server that provides cognitive persistence tools for AI agents.

Your task is to analyze the ENTIRE codebase and produce a detailed audit report with the following sections:

## 1. INVENTORY

For every file in the project, document:
- **Path**: Full path from project root
- **Purpose**: What this file does (1-2 sentences)
- **Type**: source | test | config | script | documentation | generated
- **Dependencies**: What it imports/requires
- **Dependents**: What imports/requires it
- **Last meaningful change**: If discernible from comments/structure

Format as a table or structured list.

## 2. ORPHANS

Identify:
- **Dead code**: Functions, modules, or files that are never called/used
- **Stale documentation**: Docs that reference features/patterns that no longer exist
- **Duplicate functionality**: Multiple implementations of the same thing
- **Commented-out code**: Blocks that should be removed or restored
- **Unused dependencies**: Cargo dependencies not actually used

For each orphan, provide:
- Location
- What it is
- Why you believe it's orphaned
- Recommendation (delete / investigate / restore)

## 3. INCONSISTENCIES

Look for:
- **Naming conventions**: snake_case vs camelCase, prefixes that don't match
- **Error handling**: Different patterns in different modules (anyhow vs custom, Result vs panic)
- **Logging patterns**: tracing::info vs println, inconsistent log levels
- **Response formats**: MCP tool responses that don't follow the same structure
- **Code style**: Modules that look different from others

For each inconsistency:
- What the inconsistency is
- Where it occurs (list all locations)
- What the dominant pattern is
- Recommended resolution

## 4. QUESTIONS

Things you cannot determine the purpose of:
- Files that seem purposeless
- Code paths that don't make sense
- Configurations that seem wrong
- Anything that needs human clarification

For each question:
- What you're confused about
- Where it is
- Your best guess at what it might be
- Why it matters

## 5. ARCHITECTURE OBSERVATIONS

High-level observations:
- Overall code organization assessment
- Coupling concerns
- Potential simplifications
- Technical debt hotspots
- Things that are well-done and should be preserved

---

## Key Directories to Analyze

- `src/` - Main Rust source code
- `src/tools/` - MCP tool implementations
- `src/server/` - Server infrastructure
- `scripts/` - Shell scripts and utilities
- `docs/` - Documentation
- `tests/` - Test files (if any)
- `Cargo.toml` - Dependencies
- `.env.example` - Configuration

## Context

This project:
- Is an MCP (Model Context Protocol) server
- Provides tools: think, search, remember, wander, maintain, call_gem, rethink, corrections
- Uses SurrealDB for persistence
- Uses OpenAI embeddings for semantic search
- Is part of the larger LegacyMind project

The goal of this audit is to prepare for a cleanup phase. We want to remove cruft, standardize patterns, and ensure everything has a clear purpose.

Be thorough. Be specific. When in doubt, flag it as a question rather than assuming.
```

---

## Usage

Run this prompt with Gemini (Flash or Pro) with the entire codebase loaded:

```bash
# From surreal-mind directory
gemini -m gemini-3-pro "$(cat docs/tasks/20260114-administrative-standdown/gemini-audit-prompt.md)" .
```

Or use the Web interface with file upload.

---

## Expected Output

Gemini should produce a structured report that gets saved to [audit-findings.md](audit-findings.md) for tracking and action.
