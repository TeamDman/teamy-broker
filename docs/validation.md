# Validation scope

All credential values and item references used by automated verification are synthetic. Default tests exercise injected approval/adapters/storage, actual isolated Windows named pipes, multiple clients, single-instance contention, multi-process autostart, denial, timeout cancellation, fixed 15-minute expiry, delayed cleanup, wall-clock rollback protection, purge races, graceful stop, protocol capability mismatch, provider conflicts and diagnostic redaction.

Separately reviewed ignored native tests exercise current-user DPAPI and Credential Manager with random synthetic targets, Task Scheduler registration verification, failed-publication rollback, foreign-file preservation and generation-safe replacement cleanup. Tasks and credentials are explicitly removed and checked. They do not invoke op or use actual vault data.

No automated test silently approves a real native dialog. Compiling native approval and the real op adapter does not verify interactive 1Password integration; this remains a human acceptance check using intentionally selected data. Build/test operations do not install a production daemon.
The parent-approved final template delta was verified against both manifest hashes. The broker entrypoint matches the final shared boundary hash D0F039A90E70620D5F6E8935C00A6FAA7825AF38368E02390801DFF8E2AD534F; its new export was integrated while preserving broker-specific modules and dispatch.

## Verified V1 checkpoint

- Nightly rustfmt check: passed.
- Workspace Clippy, all targets and all features with warnings denied: passed.
- Workspace all-feature build and synthetic example builds: passed.
- Default safe workspace tests with extended observability: 51 passed; 3 native tests intentionally ignored.
- Separately reviewed isolated Windows native backend tests: 3 passed, including a fresh broker instance rejecting an earlier lease; exact synthetic credentials and task IDs were removed and checked.
- Template lint-policy regression cases: 14 passed.
- Foreground synthetic consumer: passed; only byte count printed.
- Publication scan: passed seven token/private-data patterns, plus both package license declarations and complete MPL-2.0 text. A pattern scan does not detect every possible secret format.

Interactive native approval and real 1Password desktop authorization remain untested acceptance paths. No production daemon was deployed.