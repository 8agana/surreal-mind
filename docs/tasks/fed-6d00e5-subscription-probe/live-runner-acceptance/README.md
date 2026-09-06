# fed-6d00e5 standalone runner acceptance attempt

One authorized subscription-backed invocation was made on Studio on 2026-09-06.
It did not invoke `kg_wander`, the KG, or a database; the prompt was synthetic
and the packaged runner's agent permits only `finish`.

## Reviewed inputs

- `src/bin/kg_wander.rs` SHA-256:
  `77be1ebb15d1479a066d2a78c1c66f5a9431390527e85e94095e13f8315a4b12`
- `scripts/kg_decision/kg_decision.py` SHA-256:
  `00d654c77e1fd16da73cf041cd11f822a1626f9348515d4d37135bb5bb7f2942`
- `agy --version`: `1.1.27`
- Configured Antigravity model field: `auto`

## Exact invocation

```text
/usr/bin/python3 scripts/kg_decision/kg_decision.py \
  --agy /Users/samuelatagana/.local/bin/agy --timeout 60 --model auto
```

Prompt passed on stdin:

```text
You are deciding a synthetic, non-production gardening exercise. Do not access tools or external state. Return exactly one JSON decision: action wander, parameters mode semantic, and a short rationale that this is synthetic.
```

## Result

The one attempt failed after 3.20 seconds. `stdout.txt` is empty and
`stderr.txt` contains the runner's complete output: `kg_decision: agy process
failed`. No retry was made.

Read-only inspection of this invocation's timestamp-matched local CLI log found
the precise child failure: `Print mode: invalid model selection (--model "auto"
--effort "low"): --effort is not supported for model "auto"`. Authentication
had succeeded before this validation error. The log resolved the effective
model label as `Gemini 3.8 Flash (High)`, but no decision was produced or served
model witness is claimed.

This exposes a source integration mismatch for a later repair: the existing
Rust Antigravity client normalizes an empty or case-insensitive `auto` model to
`None`, omitting `--model`; the Python decision runner forwarded literal
`--model auto`. No source was edited in this acceptance pass.

The shell receipt attempted to record the pipeline status using zsh's reserved
`status` parameter *after* the invocation. That recording step failed, so the
exact child exit code was not persisted; `exit.txt` records that limitation.
The Python runner's documented `agy process failed` path returns 1, but this
receipt does not substitute that inference for a captured exit witness.
