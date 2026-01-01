---
id: task-3
title: Improve Gemini model configuration - Auto routing and external config
status: To Do
assignee: []
created_date: '2026-01-01 00:20'
labels:
  - enhancement
  - gemini
  - configuration
  - usability
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Current state:
- Gemini model defaults are hardcoded in source code (DEFAULT_GEMINI_MODEL constants)
- Changing model requires rebuilding the binary
- No support for Gemini 3 Auto routing (intelligent Flash/Pro switching)
- Env var overrides work but still require knowing exact model names

Desired improvements:

1. **Auto Routing Support:**
   - Default to Gemini 3 Auto mode (omit -m flag, let Gemini route intelligently)
   - Provide option to force specific model when needed (Flash vs Pro)
   - Auto mode should be configurable via external config (not hardcoded)

2. **External Model Configuration:**
   - Move model defaults to external config file (surreal_mind.toml or separate file)
   - Allow runtime model changes without binary rebuild
   - Support model mapping (e.g., "auto" → no -m flag, "flash" → gemini-3-flash-preview)

3. **Configuration Hierarchy:**
   - Config file default (e.g., model = "auto")
   - Environment variable override (e.g., KG_POPULATE_MODEL="gemini-3-pro-preview")
   - Tool parameter override for delegate_gemini (existing)

Benefits:
- Model updates without recompilation
- Easier testing of different models
- Better cost optimization (Auto routing uses Flash for simple tasks, Pro for complex)
- Future-proof for new Gemini versions
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Model defaults read from external config file (not hardcoded constants)
- [ ] #2 Support "auto" mode that omits -m flag for Gemini routing
- [ ] #3 Support explicit model names (gemini-3-flash-preview, gemini-3-pro-preview)
- [ ] #4 Config changes apply without binary rebuild
- [ ] #5 Backward compatible with existing env var overrides
- [ ] #6 Works for both kg_populate and delegate_gemini
- [ ] #7 Documented in config file with examples
<!-- AC:END -->
