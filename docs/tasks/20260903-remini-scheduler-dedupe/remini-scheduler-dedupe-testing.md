# REMini scheduler deduplication — acceptance

**Verdict:** PASS for immediate scheduler topology. The next 01:00 run remains
the functional acceptance witness for single-run behavior.

## Immediate checks

| Check | Result |
|---|---|
| Standalone `launchctl bootout` | PASS, exit 0 |
| Standalone label absent afterward | PASS, `launchctl print` exit 113 / service not found |
| Active standalone `.plist` absent | PASS |
| Preserved `.plist.disabled` exists and parses | PASS |
| Preserved standalone bytes equal pre-change bytes | PASS |
| Nightly-shift label remains loaded | PASS |
| Nightly-shift plist bytes unchanged | PASS |
| Full nightly-shift `launchctl print` unchanged | PASS |

## Artifact identities

- Preserved standalone plist SHA-256:
  `da531a8831ad86eafbfd1be401978ee2a16d8c75c5c5c27afe764b763955ea9e`
- Nightly-shift plist SHA-256:
  `beb01ce3f109565295a31e1dd0aa80505f51b7b24c2785dce4fe8f444fd0bdd8`

## Deferred functional witness

The next scheduled 01:00 fire must show one attributable REMini Phase 1 run
from nightly-shift and no standalone run. REMini exit 0 is not sufficient:
inspect `.summary.tasks_failed` and the six `.task_details` results. The known
wander/Antigravity permission defect is tracked separately as `fed-6d00e5`.
