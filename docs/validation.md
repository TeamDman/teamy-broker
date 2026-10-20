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
## Earlier protocol V2 repair checkpoint

At this earlier checkpoint the local consumer integration used the sibling SDK and no repair commit or push had been made; these results are historical.

- Nightly rustfmt check, strict all-target/all-feature workspace Clippy, example builds and all-feature workspace build: passed.
- Safe workspace tests with extended observability: 60 passed; the 3 native persistence tests remained ignored during this repair.
- Captured-output regression: the synthetic consumer reached process exit and output EOF while its independently running broker was still alive; the test then stopped and verified the broker.
- Isolated named-pipe tests distinguish user denial, approval timeout and upstream read failure as typed, redacted errors. Invalid peer failure payloads cannot enter SDK error messages.
- Protocol V1 and missing V2 capability peers fail compatibility before autostart.
- Publication pattern and MPL-2.0 scan: passed. No repair publication was performed.

The repair used injected approvals, fake credential adapters and isolated pipe endpoints only. It did not retry interactive credential acquisition or make a consumer API request. The corrected debug binaries were installed without deploying a persistent daemon.

## Source review on 2026-10-20

Independent reviews covered broker authorization/publication, phone pairing and revocation, transport, Android/Java, platform UI and the shared SDK/handoff protocol. The review fixed unbounded post-connect phone control/authorization exchanges by applying the configured request deadline capped at five seconds, with isolated stalled/partial-response regressions.

- Nightly formatting and strict offline all-target/all-feature workspace Clippy passed.
- Offline all-feature workspace binaries/examples build and all 14 lint-policy fixtures passed.
- Safe workspace tests with extended observability: 182 passed, 4 native/device tests left ignored. Three logging tests that read the default live broker endpoint were excluded for isolation.
- Standalone thin SDK tests: 4 passed. Pinned Java pairing tests: 58 passed. Android host policy/lifecycle/input tests passed.
- Source inventory secret-pattern scan found no token/private-key patterns. Intentional synthetic references, deployment paths and non-secret executable peer allowlists were reviewed separately; captured historical evidence and local device receipts were excluded from the commit.

Tests used synthetic values, isolated pipes, temporary state and loopback peers. No real credentials, account connections, store/scheduler tests, live daemon changes, installation, restart or push occurred. APK packaging and physical-device acceptance were not performed. Interrupted final enrollment acknowledgments can leave one-sided trust and require explicit coordinated revoke/rearm; see [phone approvals](phone-approvals.md).
