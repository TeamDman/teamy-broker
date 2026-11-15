# Validation scope

Use synthetic values, isolated endpoints and temporary state for automated checks. Build/test success does not install or update a resident broker, arm enrollment, replace phone trust, or exercise a real vault. Current remediation commands, results and unresolved failures belong in [the active plan](repository-remediation-plan.md); this page describes what each form of evidence establishes.

## Host verification

The reproducible offline command sequence and Java/SDK prerequisites are in [phone approvals](phone-approvals.md#dependencies-and-offline-checks). Build examples before running tests: provider cancellation launches `synthetic_op_wait`, and logging launches `synthetic_logging_cli`. Both fixtures own their processes and endpoints. Cargo's `--offline` prevents dependency downloads; it does not restrict a test's runtime network or credential access.

| Boundary | Existing proof and limits |
| --- | --- |
| Broker and SDK | `broker_contract`, `ipc_contract`, `failure_contract`, `autostart_capture`, `shutdown_ipc` and `shutdown_status` exercise synthetic acquisition, fixed leases, incompatible peers, captured-output EOF, cleanup ownership and shutdown admission through isolated Windows pipes. They do not establish interactive 1Password behavior. |
| Shared approval and credential supply | `approval_preview`, `shared_preview`, `provider_supply_cancellation` and broker unit tests exercise decision races, cancellation, publication checks, synthetic fixture isolation and owned provider-process termination. A winning decision alone must not bypass revocation or original expiry at cache publication. |
| Phone transport and pairing | Phone unit/integration tests, `phone_control_deadlines`, mobile contract tests and `pairing/java/build.ps1 -Test` exercise authenticated frames, independent official-peer interoperability, stale/replayed requests, deadlines and lifecycle state. Loopback/JVM checks do not prove physical Android networking or hardware key behavior. |
| Android policy | `android/check-java.ps1` runs host tests for input bounds, buffer ownership, fixture admission, durable selection, generation checks, deadlines and viewport policy. Pure geometry tests do not render an Android IME. The durable-selection fake does not inject a storage fault into an Android device. |
| Build and output | Nightly formatting, strict workspace Clippy, lint fixtures, manifest/version checks and output/logging tests validate their named source or process contracts. None constitutes a security audit. |

Tests marked `ignore` need individual review before execution. Native persistence checks can create synthetic Credential Manager entries and scheduled tasks; native UI checks can create private desktops or dialogs. Verify cleanup of only the objects the test owns. Do not infer permission to touch live state from an ignored test's presence.

## Device and native UI acceptance

The following distinctions must remain visible when recording a result:

- Native dialog visibility on a private desktop proves that shared presentation code can show the window under those startup conditions. It does not prove foreground attention or sound on the user's input desktop. Keep hidden-startup, cancellation and teardown checks in the native fixture.
- A packaged APK manifest and signature describe that artifact. They do not prove which APK is installed or that the physical device uses a hardware-backed key. The public debug signer is unsuitable for real credentials.
- Foreground-service presence, notification delivery, screen-off/Doze behavior, force-stop, reboot and reconnect are separate observations. A successful foreground preview does not establish sustained background reliability.
- A masked native field and Autofill integration need Android UI checks. An `op://` reference, item navigation link, pasted value or Autofill result does not attest that a value came from the intended vault item.
- Device acceptance must identify the tested source/artifact and workflow without putting private peer identities, account/item metadata, comparison codes or credentials in shared docs.

Historical October 2026 notes report a narrow physical-phone preview acceptance: phone approval closed the desktop dialog, desktop approval removed the transient phone notification while the app was backgrounded, and an idle desktop restart reconnected without pairing again. Separate notes report the focus-visibility repair and an isolated Android emulator check that exposed and corrected password masking. These are historical observations, not current-source validation. Extended sleep/Doze, screen-off, force-stop/reboot, physical-device credential supply and provider/item-origin behavior remain unqualified by those records. The cancelled manual bootstrap attempt was a usability rejection, not a diagnosed connection failure.

## Evidence and provenance

Keep execution history in the active plan, with the exact command, source revision or relevant dirty delta, target, outcome and limits. Do not copy old test totals, installed PIDs, artifact hashes or former activation approvals forward as current state. Public documentation should contain reusable contracts and synthetic examples; raw device/install receipts belong in ignored local evidence.

The earlier SDK installation record associates the accepted installer text with specific platform/build-tool packages. Raw repository XML, installed package records, decoded accepted text, the acceptance hash and the separately captured web agreement have different evidentiary roles. Preserve exact bytes and their manifest together; whitespace normalization is a comparison operation, not a reason to replace an accepted text or assert legal equivalence. Existing acceptance records do not accept a future license or install a new package.

Keep `pairing/java/vendor-lock.json`, `pairing/java/dependencies-lock.json` and the vendored UKEY2 `LICENSE`/`NOTICE` in source control. Reorganization of SDK receipts must not remove them. Retained hashes establish which inputs were inspected; they do not establish authorship, legal approval for every dependency or application security.
