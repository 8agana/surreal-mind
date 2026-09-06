# Subscription KG decision runner — candidate

Run `python3 kg_decision.py --agy /absolute/path/to/agy` with the prompt on
stdin. Output is one validated JSON decision; failures emit no decision.
Tests: `python3 -m unittest -v test_kg_decision test_subprocess`.

The wrapper creates an isolated workspace containing the versioned custom
agent and uses Agy's existing subscription authentication. It removes inherited
API-key variables and SURR_ENV_FILE; it copies no credentials. Agy's normal
subscription session/bookkeeping remains outside the temporary workspace.

The agent permits only finish. Explicit --new-project ensures its definition
is loaded. Behavioral positive/negative controls are preserved under
docs/tasks/fed-6d00e5-subscription-probe. The init.tools field advertises a
broader inventory than the selected agent; it is not used as a capability
attestation. Unexpected tool events cause result rejection, but that detection
occurs after the tool attempt; it is not itself a sandbox.

The runner consumes result.structured_output only, checks session correlation,
rejects incomplete/duplicate results and duplicate JSON keys, and validates
action-specific parameters. It enforces a timeout and a polled output-size
bound. Output may briefly exceed the bound between polls. No retries.

Acceptance this milestone: ten offline parser/subprocess tests cover valid
output, malformed stream rejection, child failure, output limits, timeout, and
descendant cleanup. No KG action is executed by these fixtures.

Rust `kg_wander` integration is opt-in through `KG_WANDER_DECISION_RUNNER` and
is permitted only with the Antigravity provider; Gemini rollback remains direct.
The runner matches the Rust Antigravity client for model forwarding: an empty
or case-insensitive `auto` model omits `--model`; explicit model values are
trimmed and forwarded.
This candidate is not installed or used by the nightly scheduler.
