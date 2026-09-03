# REMini scheduler deduplication — implementation

## Applied operation

At `2026-09-03T21:47:21Z` on Studio:

1. Captured SHA-256 and full `launchctl print` for both loaded jobs.
2. Ran `launchctl bootout gui/501/dev.legacymind.remini` once.
3. Renamed the installed plist from
   `~/Library/LaunchAgents/dev.legacymind.remini.plist` to
   `~/Library/LaunchAgents/dev.legacymind.remini.plist.disabled`.
4. Added a warning to the retained repository plist that it is superseded and
   must not be installed concurrently with nightly-shift.

No retry was required.

## Rollback

Rollback is recoverable and uses the exact preserved bytes:

1. Verify the disabled plist still has SHA-256
   `da531a8831ad86eafbfd1be401978ee2a16d8c75c5c5c27afe764b763955ea9e`.
2. Rename it back to `~/Library/LaunchAgents/dev.legacymind.remini.plist`.
3. Run `launchctl bootstrap gui/501 ~/Library/LaunchAgents/dev.legacymind.remini.plist`.
4. Verify `launchctl print gui/501/dev.legacymind.remini` succeeds and matches
   `before-launchctl-remini.txt` apart from runtime counters/identifiers.

Rollback is not authorized merely by this documentation; use it only if the
deduplication is intentionally reversed or nightly-shift cannot provide the
required maintenance path.
