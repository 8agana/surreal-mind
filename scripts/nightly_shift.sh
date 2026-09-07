#!/bin/bash
# Nightly Agent Shift - Launched at 1am by launchd
#
# Phase 1 is deterministic maintenance. Do not delegate required maintenance to
# an agentic CLI; if a task hangs, REMini's per-task timeout must own the result.
# Phase 2 is optional reflection through Antigravity (`agy`).

set -uo pipefail

cd /Users/samuelatagana/Projects/LegacyMind/surreal-mind || exit 1

export SURR_ENV_FILE=/Users/samuelatagana/Projects/LegacyMind/surreal-mind/.env
export ANTIGRAVITY_CLI_BIN=${ANTIGRAVITY_CLI_BIN:-/Users/samuelatagana/.local/bin/agy}
# Opt-in only for the REMini wander child; keep Gemini rollback direct when
# this binding is removed. Do not move this into the global .env.
export KG_WANDER_DECISION_RUNNER=/Users/samuelatagana/Projects/LegacyMind/surreal-mind/scripts/kg_decision/kg_decision.py
# Nightly-only default; explicit operator values remain authoritative.
export KG_WANDER_TIMEOUT_MS=${KG_WANDER_TIMEOUT_MS:-120000}
export PATH="/Users/samuelatagana/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

LOG="logs/nightly_shift.log"
REPORT="logs/remini_report.json"
REMINI="./target/release/remini"
SHIFT_STARTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

mkdir -p logs

log() {
  printf '[%s] %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"
}

log "nightly_shift start: deterministic REMini maintenance + bounded agy reflection"

if [ ! -x "$REMINI" ]; then
  log "FAIL: $REMINI is missing or not executable"
  exit 1
fi

log "phase1 start: $REMINI --all --timeout 1800"
"$REMINI" --all --timeout 1800
remini_status=$?

if [ "$remini_status" -eq 0 ]; then
  log "phase1 complete: REMini exited 0"
else
  log "phase1 failed: REMini exited $remini_status"
fi

if [ -f "$REPORT" ]; then
  log "phase1 report: $REPORT"
else
  log "phase1 report missing: $REPORT"
fi

if [ -x "$ANTIGRAVITY_CLI_BIN" ]; then
  SHIFT_SUMMARY="Nightly shift started at ${SHIFT_STARTED_AT}.

Phase 1 was run deterministically with:
  ./target/release/remini --all --timeout 1800

Exit code: ${remini_status}
Report path: ${REPORT}

If useful, summarize what this maintenance result means and note one optional curiosity or follow-up. Keep it concise. Do not run tools."

  log "phase2 start: bounded agy reflection"
  "$ANTIGRAVITY_CLI_BIN" --print "$SHIFT_SUMMARY" --print-timeout 120s --sandbox
  agy_status=$?
  log "phase2 complete: agy exited $agy_status"
else
  log "phase2 skipped: ANTIGRAVITY_CLI_BIN not executable: $ANTIGRAVITY_CLI_BIN"
fi

log "nightly_shift complete"
exit "$remini_status"
