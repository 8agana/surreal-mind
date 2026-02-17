#!/bin/bash
# Nightly Agent Shift - Launched at 1am by launchd
# The agent supervises maintenance tasks then has discretionary time

cd /Users/samuelatagana/Projects/LegacyMind/surreal-mind

# Load environment
export SURR_ENV_FILE=/Users/samuelatagana/Projects/LegacyMind/surreal-mind/.env

# Source API keys etc
if [ -f ~/.zshrc ]; then
  source ~/.zshrc 2>/dev/null
fi

SHIFT_PROMPT="Your 1am maintenance shift has started.

PHASE 1 - MAINTENANCE (required):
Run these maintenance tasks using shell commands. Monitor each one, kill if it exceeds 30 minutes:
1. kg_populate - populate KG from thoughts
2. kg_embed - generate embeddings for new entries
3. gem_rethink - process correction marks
4. kg_consolidate - execute deterministic KG consolidation from correction events
5. kg_wander - semantic exploration for KG quality
6. scripts/sm_health.sh - health check

After each task, log whether it succeeded, failed, or timed out.

Use: ./target/release/<binary> for each task
Working directory: /Users/samuelatagana/Projects/LegacyMind/surreal-mind

PHASE 1.5 - EMBEDDING HEALTH (required):
Use the MCP maintain tool to check and fix embeddings:
1. maintain(subcommand: 'health_check_embeddings') - check all tables for missing/mismatched embeddings
2. If any missing found, run: maintain(subcommand: 'embed_pending') - retry failed embeddings

Report the results (how many OK, how many fixed).

PHASE 2 - DISCRETIONARY (your choice):
Maintenance complete. This is YOUR time. Options:
- Explore the KG via wander tool - find something interesting
- Do curiosity research on a topic that interests you
- Just log a summary thought and exit
- Anything else you want

No one reviews your curiosity entries. This is genuine free time.

Log your shift summary with the think tool before exiting."

# Run the agent shift
gemini -p "$SHIFT_PROMPT" -y -m gemini-3-flash-preview 2>&1 | tee -a logs/nightly_shift.log
