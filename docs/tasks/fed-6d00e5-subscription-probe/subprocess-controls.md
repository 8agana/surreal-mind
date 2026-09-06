# fed-6d00e5 offline subprocess milestone

Added `scripts/kg_decision/test_subprocess.py`. All fixtures use generated
Python executables in temporary directories, not Antigravity or API services.

Acceptance: the parser plus subprocess suite has ten offline tests, using
`/usr/bin/python3 -m unittest -v test_kg_decision test_subprocess`.

Each control checks the recorded child PID is absent and its temporary working
directory has been removed after return or rejection. Cases: valid terminal
decision, exit 7, stdout above 2 MiB, a sleeping TERM-ignoring child, and both
timeout and parsing-rejection paths with a surviving-descendant attempt. The timeout control uses
timeout=1; the runner adds five seconds of grace and the test requires
completion within ten seconds. Rust's adapter test separately asserts its
shared Python+descendant process group is killed after its bounded deadline and
after explicit async cancellation.

Limits: no stderr-only limit control, no real provider invocation, and no
attempt to contain a malicious child that calls `setsid`. This does not witness a repaired
nightly wander, Rust integration, deployment, or a KG action. Existing candidate
and probe artifacts are still staged in the isolated worktree. Next: integrate
the validated runner with the Rust decision path and review error propagation.
