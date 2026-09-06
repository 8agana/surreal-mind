# fed-6d00e5 integrated KG acceptance

The sole actual integrated `kg_wander` invocation ran in the isolated Studio
worktree under a stdlib Python controller with a 120-second deadline. The prior
missing-`gtimeout` launcher failure did not start the target and is recorded
separately.

## Result

Exit 0; real time 13.28 seconds; stderr empty. The run completed one normal KG
step (`KG_WANDER_MAX_STEPS=1`) with the reviewed opt-in runner. It began at
existing node `jdcylkefvj8bbbcdn1ix`, selected a read-only semantic `wander`,
and returned existing node `8f32e00c-14b7-439e-8491-d1c8ed4efee3`.

Because the selected action was read-only wander, the returned existing node is
the action witness; no write-action readback was applicable. No retry,
deployment, restart, config change, or commit followed.

## Hashes

- Rust source: `77be1ebb15d1479a066d2a78c1c66f5a9431390527e85e94095e13f8315a4b12`
- Python runner: `1ea4ea0b848c49321830676b986fe38fabe3c03fee1416ddd1e342cfd0e0b9e8`
- Debug binary: `f1c94fb6a31efb5dd79e766ddf00cd565bcb9f02b79f74a1d7774b3dd5c11695`
