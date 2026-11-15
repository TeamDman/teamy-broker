# Phone approvals

The resident broker supports an explicitly enrolled phone for shared approval previews. Enrollment alone grants no credential access. A separate, short-lived authorization is required for each exact credential request. Building this source does not install an APK or broker, restart a daemon, arm pairing, or change stored trust.

## Dependencies and offline checks

The Windows broker uses the pinned official Google Java UKEY2 implementation for initial human comparison, then TLS 1.3 with mutual certificate authentication for enrolled connections. The existing JDK must provide `java`, `javac`, and `jar`. `pairing/java/dependencies-lock.json` pins the local dependency files; `vendor-lock.json` pins the retained upstream sources. Populate the ignored `pairing/java/deps` directory with the separately obtained matching files before building. The build script verifies hashes and fails on missing or mismatched inputs; it downloads nothing and executes no upstream build script. Keep the source locks and vendored UKEY2 `LICENSE`/`NOTICE`; see [pairing ownership and provenance](mobile-handoff.md#pairing-implementation-and-provenance).

Run from the repository root with the required Rust toolchains and dependencies already available:

```powershell
.\pairing\java\build.ps1 -Test
.\android\check-java.ps1
rustup run nightly -- cargo fmt --all -- --check
cargo clippy --offline --workspace --all-targets --all-features --no-deps -- -D warnings
cargo build --offline --workspace --bins --examples
cargo test --offline --workspace --features extended_observability -- --test-threads=1
```

Build the examples before tests: provider cancellation launches `synthetic_op_wait`, and logging tests launch `synthetic_logging_cli`. Keep tests marked `ignore` excluded unless their native storage, scheduler, or private-desktop requirements have been separately reviewed. Normal tests use synthetic adapters, isolated pipe endpoints, temporary state, and loopback peers.

Android APK packaging additionally requires an existing approved Android SDK platform 35/36, build tools, and an explicitly selected signing key. `android/build-debug.ps1` produces the synthetic debug package. `android/build-release.ps1` requires an existing private signing identity, password environment-variable names, and its expected fingerprint; it does not create a key or install/pair a device. The public debug signer is unsuitable for real credentials.

The broker reads `phone-profile.json` at runtime through `PhoneProfile::load(&home)`. The caller supplies the application home. The file must identify both peers and an absolute Java executable path. See the schema rules below. Missing or invalid configuration rejects phone network activation.

The legacy `.\use-private-phone-profile.ps1` helper reads the ignored `private-phone-profile.local.json` into the current shell's environment. It requires string values for its seven `TEAMY_BROKER_*` fields. It does not install or migrate the broker's runtime profile.

The listener checks the actual socket peer and local node through Tailscale metadata against that exact configured scope; there is no arbitrary-host or all-interface fallback. The broker also requires the explicitly selected absolute Java executable at runtime. Runtime artifact lookup currently assumes the source-tree `pairing/java` layout.

## Phone profile schema

New runtime profiles must include the top-level field `"schema_version": 1`. Version 1 keeps the existing fields:

- `TEAMY_BROKER_DESKTOP_IP` and `TEAMY_BROKER_PHONE_IP`: distinct IPv4 addresses within the Tailscale range
- `TEAMY_BROKER_DESKTOP_NODE` and `TEAMY_BROKER_PHONE_NODE`: node identities
- `TEAMY_BROKER_TAILNET_USER`: a nonzero user ID, written as a JSON number; quoted numeric IDs remain accepted
- `TEAMY_BROKER_PHONE_NAME`: the expected phone node name
- `TEAMY_BROKER_JAVA`: an absolute Java executable path

The reader checks the schema version before decoding the profile fields. Missing `schema_version`, or an explicit value of `0`, selects the legacy layout. That layout has the same seven fields and upgrades to version 1 without changing the peer scope. The migration report identifies the source version and includes a diagnostic. File loading logs that diagnostic and leaves the file unchanged.

`PhoneProfile::parse_json` returns the runtime profile and migration report. `PhoneProfile::to_json` validates the profile and serializes version 1, including its version field. To update an existing file, serialize the parsed profile and save the result explicitly. Reading never rewrites configuration.

Malformed versions and unsupported versions fail. The reader does not retry them as legacy profiles. Both supported layouts enforce the same peer and Java-path validation. Configuration files remain limited to 8192 bytes, and load failures retain their underlying I/O, UTF-8, and JSON errors.

The implementation follows the [Facet JSON versioning approach](https://teamdman.github.io/teamy-docs/facet-json-versioning.html): version preflight, separate schema types, and an explicit upgrade. Future schema changes must add a version-specific type and conversion, with historical-input and unsupported-version tests.

## Explicit enrollment

Phone commands contact an already-running compatible resident through its secured same-user named pipe; they never autostart it. Coordinate the phone and desktop before arming enrollment. Inspect the current relationship first:

```powershell
teamy-broker phone-list
teamy-broker phone-status
teamy-broker phone-pair
```

`phone-pair` returns an opaque `pairing_attempt_id` and arms one five-minute window. A paired phone cannot be silently replaced: replacement requires explicit revocation first. Starting the phone's pairing action must occur within that original window; neither inspection nor confirmation renews it.

Compare every group of the code shown independently on the phone and desktop, then explicitly confirm on both devices. The native desktop dialog defaults to rejection. The following optional CLI controls address only the exact attempt UUID returned above:

```powershell
teamy-broker phone-pair-inspect --attempt-id ATTEMPT_UUID
teamy-broker phone-pair-confirm --attempt-id ATTEMPT_UUID --comparison-stdin
teamy-broker phone-pair-cancel --attempt-id ATTEMPT_UUID
```

Inspection intentionally displays the active comparison and expiry. Treat that output as local pairing material. Confirmation reads the independently supplied phone comparison from stdin: one line, at most 128 bytes, within five seconds; Enter or piped EOF terminates input. Do not put the comparison in command arguments, logs, or shell history, and never automatically feed desktop inspection output into confirmation. There is no latest-attempt, wildcard, or blind-confirm operation. Three wrong codes close the comparison; stale, expired, replayed, or cancelled attempts reject. Native and CLI confirmation share one atomic decision. Cancellation affects only the identified pending comparison.

After enrollment, start the phone's approval connection from the visible app with notifications enabled. Its nonexported foreground service has an ongoing Stop notification and bounded reconnect backoff. It is not boot-started or sticky. Phone process death/force-stop requires reopening and starting it. Sleep, Doze, OEM restrictions, or network loss can delay notifications; they do not extend approval deadlines.

Desktop identity state uses current-user DPAPI; Android uses a nonexportable Keystore key and a transactional private identity selection. Mutual TLS checks the enrolled leaf identity. Application messages bind the relationship generation, fresh connection session, increasing sequence, and exact request ID. TLS resumption and early data are disabled. This remains a cooperative same-user broker boundary, not remote APK attestation or protection against a compromised OS.

Android publishes the selected key alias and desktop pin together through `IdentitySelectionStore` using a SQLite transaction with `synchronous=FULL`. `DurableIdentitySelection` reads committed state instead of caching an uncommitted candidate. Legacy preferences remain a read-only fallback until a selection record exists; Forget commits an empty selection before key deletion so that fallback cannot restore old trust. Host tests exercise failure/restart/retry through an injected store, not physical-device storage fault injection.

## Connection deadlines

Human comparison and network operations have different budgets. Enrollment retains its original five-minute window. The current resident transport uses a ten-second idle-first-byte wait and a separate five-second deadline to assemble the rest of a frame. A slow byte stream cannot continually renew frame assembly. Server writes are bounded to five seconds. See `src/broker/phone/{timing,message_read,transport}.rs`.

Android's `PhoneTiming` sets a three-second TCP connect timeout, a 25-second connection operation, a 30-second exchange and a five-second outbound write budget. These whole-operation budgets account for the server's peer checks. `OperationDeadline` cancels completed timers without closing a later replacement socket. Reconnect creates a fresh session and sequence and reconciles current state; it never renews a request's 60-second lifetime or a credential's 15-minute lease. A deadline category identifies the observed timer boundary, not the underlying cause of a network disconnect or device scheduling delay.

## Previews and credential actions

`teamy-broker preview` is proposal-only. `teamy-broker preview --show-test-dialog` opens a deliberate no-vault test request. Desktop and phone contend on one authoritative request; the first valid decision wins. X/Esc dismisses the desktop presentation while the shared request remains pending. Cancellation and the original 60-second wall/monotonic deadline reject late decisions. Previews create no credential lease or provider read.

`phone-authorize` grants the next exact matching credential request once, within five minutes. It requires `--account`, `--secret-ref`, `--requesting-app`, `--purpose`, and `--destination`; the backend is always Memory. Without `--supply`, it allows the shared request to ask the desktop's fixed 1Password adapter for that exact item. `--supply` additionally permits explicit value entry on the phone or desktop. Authorization itself neither reads nor supplies a credential. It is memory-only, consumed once, and cleared on restart/revocation; it does not broaden durable enrollment scope. Unrelated requests retain ordinary desktop approval.

Provider retrieval and explicit supply compete for one winning value. Values are bounded to 4,096 bytes and carried separately from JSON metadata. Revocation, cancellation, original approval expiry, and relationship generation are checked again before cache publication. A successful acquisition produces the existing fixed 15-minute memory lease from value resolution; activity does not renew it. Exact matching request metadata can reuse that lease across same-user callers; connected PID is an approval diagnostic, not persistent consumer isolation. Purge cannot revoke caller copies or upstream tokens.

The debug Android package accepts only the explicitly authorized built-in synthetic fixture. `phone-authorize --synthetic-fixture` requires every field to equal `src/broker/synthetic_credential_fixture.rs`; the consumer must use the typed `Client::acquire_fixture` operation. Fixture acquisition skips existing cached leases and cannot fall back to desktop approval or a real provider. Its provider result and accepted entered value are fixed constants. A vault or reference named "Synthetic" does not make an ordinary request a fixture. Real credential requests require a reviewed non-debuggable release package, private signer, separately enrolled identity, and explicit exact-action authorization; a build or update grants none of these.

## Revocation and interrupted enrollment

```powershell
teamy-broker phone-revoke
```

Revocation durably removes the accepted phone certificate, changes the relationship generation, clears pending per-action permissions, cancels credential approval/pairing state, and causes the resident listener/connection to close. A revoked identity must complete a fresh human comparison before reconnecting. The phone's explicit Forget action separately clears its selected desktop pin and key state; do not assume a desktop revoke performs that phone-side action.

Enrollment commits desktop trust before the final acknowledgment. Losing that acknowledgment can leave one device reporting incomplete enrollment while the other retained a relationship. Inspect both sides instead of retrying blindly. If recovery requires replacement, coordinate an explicit desktop revoke and a fresh pairing window with the intended phone identity. Do not silently restore trust, bypass comparison, or infer successful enrollment from an accepted TCP connection. A debug-to-release package change has a different Android UID and separate identity state; installation does not migrate or replace an existing relationship.

Host tests establish protocol and state-machine behavior. They do not establish hardware-backed key availability, physical-phone notification delivery, Autofill behavior, or sustained sleep/Doze reliability. No installation, daemon restart, identity replacement, or real credential action is part of the offline checks above.
