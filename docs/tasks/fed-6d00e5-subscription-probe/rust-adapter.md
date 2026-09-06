# Candidate Rust adapter (fed-6d00e5)

Studio verification: `cargo test --bin kg_wander` passed four tests, including
the fixture subprocess adapter cases; `cargo clippy --bin kg_wander -- -D
warnings` passed. Formatting and `git diff --check` passed. No live runner
or database was invoked by these tests.

`KG_WANDER_DECISION_RUNNER` selects an absolute Python runner script in
`kg_wander` only when the selected provider is Antigravity. Gemini plus the
runner is rejected before server setup, preserving the Gemini rollback. Unset
preserves the existing direct-provider path; this is staged, not deployed.
The script receives stdin, the configured Antigravity executable, and a
1–300 second timeout derived from the existing millisecond configuration.
No new provider permission is granted and no model is called in Rust tests.

Adapter checks nonzero exit, output size, exact JSON, known action, object
parameters, and nonempty rationale. Action-specific parameter validation
remains in the packaged Python runner. No legacy parser or random fallback
is used after an opted-in runner failure.

Supervision: on the Rust opt-in path, Rust starts Python in a new process group
and passes `--supervised`; Python then leaves that shared Python+`agy` group to
Rust. Rust bounds the runner at the configured timeout rounded to seconds plus
six seconds of cleanup grace, bounds combined stdout/stderr to 64 KiB both
while waiting and after exit, signals the whole group, and reaps its direct
Python child after every worker result or observed async cancellation. Input is file-backed before process start
so a runner that never reads stdin cannot block setup. Standalone Python retains
its own new session and process-group cleanup. Model forwarding
now passes the Rust-selected model through Python `--model` to Antigravity's
documented `--model` flag. Offline Rust and Python fixtures assert the exact
argument value at each boundary; this is not a served-model witness.
Review limits: this only supervises descendants that remain in Rust's shared
process group. A deliberately escaping child (`setsid`) or an uninterruptible
kernel task cannot be guaranteed reaped. Tokio task cancellation signals the
blocking worker, which observes it on its 25ms poll interval and kills the
group. In supervised mode Python does not kill its own group; its `finally`
wait can therefore last until Rust's whole-invocation deadline if its child does
not exit. Rust remains the deadline and cleanup owner.
Initial random KG
read and server setup still precede the first decision. No nightly success,
provider invocation, KG mutation, or production install is claimed here.
