#!/bin/bash
# Disposable binary-level contract for REMini's outer task supervisor.

set -euo pipefail

REPO_ROOT="${REPO_ROOT:-/Users/samuelatagana/Projects/LegacyMind/surreal-mind-wt-fedfdece5}"
REMINI="$REPO_ROOT/target/release/remini"
CHILD="$REPO_ROOT/target/release/kg_populate"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fed-fdece5-remini.XXXXXX")"
ORIGINAL_CHILD="$WORK_DIR/kg_populate.original"
ORIGINAL_PRESENT=0
SENTINEL_PID=""
DESCENDANT_PID_FILE="$WORK_DIR/descendant.pid"
CHILD_PID_FILE="$WORK_DIR/child.pid"

cleanup() {
  if [ -n "$DESCENDANT_PID_FILE" ] && test -s "$DESCENDANT_PID_FILE"; then
    pid=$(cat "$DESCENDANT_PID_FILE")
    if kill -0 "$pid" 2>/dev/null; then kill "$pid" 2>/dev/null || true; fi
  fi
  if [ -n "$SENTINEL_PID" ] && kill -0 "$SENTINEL_PID" 2>/dev/null; then
    kill "$SENTINEL_PID" 2>/dev/null || true
    wait "$SENTINEL_PID" 2>/dev/null || true
  fi
  rm -f "$CHILD"
  if [ "$ORIGINAL_PRESENT" -eq 1 ]; then
    mv "$ORIGINAL_CHILD" "$CHILD"
  fi
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

test -x "$REMINI"
if test -e "$CHILD"; then
  mv "$CHILD" "$ORIGINAL_CHILD"
  ORIGINAL_PRESENT=1
fi

cat >"$CHILD" <<'PY'
#!/usr/bin/python3
import os
import subprocess
import sys
import time
from pathlib import Path

mode = os.environ["REMINI_CONTROL_MODE"]
Path(os.environ["REMINI_CHILD_PID_FILE"]).write_text(str(os.getpid()))
if mode == "overflow":
    sys.stdout.write("O" * 262144)
    sys.stdout.flush()
    sys.stderr.write("E" * 262144)
    sys.stderr.flush()
    raise SystemExit(0)
if mode == "descendant":
    child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
    Path(os.environ["REMINI_DESCENDANT_PID_FILE"]).write_text(str(child.pid))
    print("pre-timeout stdout diagnostic", flush=True)
    print("pre-timeout stderr diagnostic", file=sys.stderr, flush=True)
    time.sleep(60)
    raise SystemExit(0)
if mode == "success":
    print("normal stdout")
    print("normal stderr", file=sys.stderr)
    raise SystemExit(0)
if mode == "failure":
    print("failure stdout")
    print("failure stderr", file=sys.stderr)
    raise SystemExit(7)
raise SystemExit("unknown control mode")
PY
chmod 755 "$CHILD"

sleep 60 & SENTINEL_PID=$!

run_remini() {
  local mode=$1
  local report="$WORK_DIR/$mode.report.json"
  local stdout="$WORK_DIR/$mode.stdout"
  local stderr="$WORK_DIR/$mode.stderr"
  : >"$stdout"
  : >"$stderr"
  REMINI_CONTROL_MODE="$mode" \
  REMINI_CHILD_PID_FILE="$CHILD_PID_FILE" \
  REMINI_DESCENDANT_PID_FILE="$DESCENDANT_PID_FILE" \
    "$REMINI" --tasks populate --timeout 1 --report-path "$report" \
    >"$stdout" 2>"$stderr" &
  local pid=$!
  for _ in $(seq 1 60); do
    if ! kill -0 "$pid" 2>/dev/null; then break; fi
    sleep 0.1
  done
  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    echo "REMini hung beyond outer control bound: mode=$mode" >&2
    return 1
  fi
  wait "$pid"
  jq -c '.task_details[0] | {success,duration_ms,stdout,stderr}' "$report"
}

overflow_report=$(run_remini overflow)
echo "overflow=$overflow_report"
printf '%s' "$overflow_report" | jq -e '.success == true and (.stdout | contains("OUTPUT_TRUNCATED")) and (.stderr | contains("OUTPUT_TRUNCATED"))' >/dev/null
echo "overflow_control=PASS"

descendant_report=$(run_remini descendant)
echo "descendant=$descendant_report"
printf '%s' "$descendant_report" | jq -e '.success == false and (.stdout | contains("pre-timeout stdout diagnostic")) and (.stderr | contains("pre-timeout stderr diagnostic")) and (.stderr | contains("TIMEOUT"))' >/dev/null
descendant_pid=$(cat "$DESCENDANT_PID_FILE")
if kill -0 "$descendant_pid" 2>/dev/null; then
  echo "descendant_after_timeout=alive (FAIL)" >&2
  exit 1
fi
echo "descendant_control=PASS"

success_report=$(run_remini success)
echo "success=$success_report"
printf '%s' "$success_report" | jq -e '.success == true and .stdout == "normal stdout\n" and .stderr == "normal stderr\n"' >/dev/null
echo "normal_success_control=PASS"

failure_report=$(run_remini failure)
echo "failure=$failure_report"
printf '%s' "$failure_report" | jq -e '.success == false and .stdout == "failure stdout\n" and .stderr == "failure stderr\n"' >/dev/null
echo "nonzero_control=PASS"

if kill -0 "$SENTINEL_PID" 2>/dev/null; then
  echo "unrelated_sentinel=alive"
else
  echo "unrelated_sentinel=DEAD" >&2
  exit 1
fi

echo "all_remini_subprocess_controls=PASS"
