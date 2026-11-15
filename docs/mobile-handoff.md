# Mobile handoff and pairing ownership

`teamy-broker-handoff` contains the experimental synthetic handoff protocol and shared pairing records. Some consumers are test fixtures; others are part of the resident phone implementation. Use the consumer map below before removing an export or changing its wire representation.

| Component | Responsibility and consumers |
| --- | --- |
| `handoff/src/{binding,envelope,pending,queue,session,wire}.rs` | Synthetic request binding, AES-GCM envelopes, bounded in-memory queue and framed transport. Mobile contract tests and paired SDK interoperability tests exercise these paths. |
| `handoff/src/{clock,clock_reading,deadline}.rs` | Wall and monotonic deadline behavior used by the synthetic queue and its race tests. |
| `handoff/src/pairing_binding.rs` | Canonical application context authenticated by both UKEY2 peers. Its `dummy` constructor supplies synthetic defaults; the record is also used by persistent enrollment. |
| `src/broker/mobile_dummy/` | Synthetic handoff adapters, queue approval and loopback TLS fixtures. Production phone code does not depend on this module. |
| `src/broker/phone/` | Resident relationship state, protected persistence, revocation, TLS connections and request authorization. See [phone approvals](phone-approvals.md). |
| `src/broker/phone/config/` | Runtime profile schemas, migration and validated peer/JDK values. |
| `src/broker/phone/tailnet/` | Exact configured peer policy and Tailscale response decoding. |
| `src/broker/phone/pairing/` | Java UKEY2 worker, comparison UI, framing and persistent enrollment. Synthetic queue and temporary preview bridges are compiled only for interoperability tests. |
| `pairing/java/` and `android/` | Pinned UKEY2 engine plus its private desktop worker, and the native Android application. Android's current package directory retains the historical `dummy` name; build identity and runtime admission determine its capabilities. |

## Synthetic handoff contract

The legacy handoff fixture accepts only the fixed `DUMMY_SECRET` and exact synthetic metadata in `handoff/src/lib.rs`. Its queue binds the request identity, nonce, account, consumer, destination, scope and expiry. Desktop and phone decisions compete for one entry; the first valid decision consumes it. Deadline checks occur before delivery as well as before consumption. Queue acknowledgment records acceptance of a decision, not successful SDK grant publication.

Synthetic acquisition reuses the broker's approval, cancellation, purge and fixed 15-minute memory-lease behavior. A later cancellation, purge or expiry can prevent publication. Purging broker state cannot revoke bytes a consumer already copied. The separate resident credential fixture uses `src/broker/synthetic_credential_fixture.rs` and `Client::acquire_fixture`; these fixtures are different contracts. A vault name or reference prefix never identifies a safe fixture.

## Manual examples

Four of the ten Windows examples are manual diagnostics. From the repository root, use `cargo run --offline -p teamy-broker --example NAME -- ARGUMENTS`, substituting the name and arguments below. Build prerequisites are in [phone approvals](phone-approvals.md#dependencies-and-offline-checks). Native UI rows open windows when activated; the resident fixture contacts the existing broker. Running an example is a separate action from building it.

| Example and arguments | Prerequisites and behavior |
| --- | --- |
| `broker_attention_dummy --show-harmless-attention` | Native UI on the current desktop, lasting at most 20 seconds. Choose **Deny**; the report records foreground ownership after one second. Audible attention needs a human observation. Omitting the exact flag prints a proposal. |
| `synthetic_dpi_dialog X Y EXPECTED_DPI` | Native UI at the supplied integer screen coordinates, closing after three seconds. Select a point on the monitor being checked and its expected DPI, for example `96` for 100% scaling. Verifies PerMonitorV2 awareness and observed dialog DPI. |
| `synthetic_credential_resident --run-isolated-fixture` | Contacts the existing per-user resident; it requires an enrolled phone and an idle broker with zero leases. It authorizes and requests the exact built-in credential fixture, then purges its own memory lease. Coordinate the phone/desktop decision UI. The only accepted values are `teamy-fixture-provider-v1` and `teamy-fixture-input-v1`; output omits the value. No exact flag means no request. |
| `synthetic_consumer --provider teamy-broker` | [SDK selection example](../README.md#sdk), using injected approval, fake storage and a random IPC endpoint. Leave `TEAMY_SYNTHETIC_CREDENTIAL_PROVIDER` unset when supplying the flag; selecting both is an error. It consumes the fixed test-adapter value, purges its lease and stops its own server, without native UI. |

The handoff tests accept only `TEAMY-DUMMY-ONLY` with the exact metadata defined in [the handoff crate](../handoff/src/lib.rs) and [synthetic request constructor](../src/broker/mobile_dummy/mod.rs). The resident fixture instead uses [its separate exact request](../src/broker/synthetic_credential_fixture.rs). These fixtures do not accept arbitrary credentials or select a provider by vault/reference name.

## Examples owned by test runners

Build these six helpers through the normal bins/examples build, then run their owning tests or scripts. Their arguments, process lifetime and endpoint setup belong to those runners.

| Helpers | Owning tests or scripts |
| --- | --- |
| `synthetic_broker`, `synthetic_autostart_consumer` | [IPC contract tests](../tests/ipc_contract.rs) and [autostart capture tests](../tests/autostart_capture.rs). |
| `synthetic_op_wait` | [Provider supply/cancellation tests](../tests/provider_supply_cancellation.rs). |
| `synthetic_logging_cli` | [Logging fixture](../tests/support/logging_cli.rs), used by `log_sinks` and `log_span_timings`. |
| `synthetic_preview_resident` | [Private-desktop presentation runner](../resources/test-hidden-startup-preview.ps1), which requires `-Execute`, `-DriverPath`, `-ExpectedDriverSha256` and `-OutputDirectory`. It owns the private desktops and child processes. |
| `mobile_tls_probe` | [JVM/Rust loopback TLS runner](../android/check-tailnet-transport.ps1), using the existing Java/Javac tools and optional `-TargetDirectory`. Despite the script name, it tests loopback and certificate-pin rejection, without activating Tailscale. |

## Pairing implementation and provenance

The current desktop worker and Android app use the official Google UKEY2 Java implementation pinned by `pairing/java/vendor-lock.json`, with archive and per-file digests. Keep the vendored `LICENSE` and `NOTICE` with that source. `pairing/java/dependencies-lock.json` records exact external artifacts and their hashes; `pairing/java/build.ps1` verifies them without downloading or running upstream Gradle scripts. Android packaging includes the UKEY2 license and notice under APK assets. Hash agreement is reproducibility evidence, not an independent security audit or a complete third-party license audit.

Both peers must confirm the same live comparison before application traffic is accepted. Role and exact application binding are authenticated inside the derived connection context. `VerifiedPairing` gives transport callers cloned handshake buffers: the official engine retains transcript arrays until key derivation. Clearing a transport buffer must not clear the engine's transcript. Independent official-peer tests in `pairing/java` cover each wrapped role; keep that regression when replacing the worker.

`pairing/java/runtime-licenses-lock.json` pins the separately supplied protobuf and jsr305 license texts under `pairing/java/licenses/`; retain those files alongside the embedded dependency notices and UKEY2 notices. Successful Java builds generate `build/runtime-artifacts.json` from current source/lock/JAR hashes. Packaging verifies that receipt and emits `package-manifest.json` with relative paths and hashes. These generated receipts describe their build/package; they do not replace the maintained source locks and license files.

The Windows runtime uses the Java artifacts in the executable's sibling `pairing` directory and a configured absolute Java executable. `package-broker.ps1` assembles that layout from verified build inputs. Host fixtures can still use the source-tree Java build; production does not depend on its checkout location.

A bounded Rust/JNI trial reproduced 50 profile-validation cases and 10,000 repeated host-JVM calls, then compiled an ARM64/API 33 library. It proved feasibility but did not justify shipping a native loader, ABI packaging and runtime dependency for two small syntax rules. The trial and its evidence remain ignored locally. Production retains Java for Android lifecycle/UI, Keystore, TLS integration and the pinned UKEY2 implementation. Makepad's Java/JNI bootstrap was a useful reference; no Makepad dependency or all-Rust claim was introduced. API 29 runtime behavior, Android ART, hardware keys and device lifecycle still require their own acceptance evidence.

## Build and validation

Use the offline commands and environment prerequisites in [phone approvals](phone-approvals.md#dependencies-and-offline-checks). [Validation scope](validation.md) distinguishes host tests, packaged artifacts and device acceptance. Building an APK or passing a JVM exchange does not establish Android hardware, Autofill, notification or sleep behavior.

SDK installer receipts and obsolete APK/runtime checkpoints are local historical evidence, not application source or current installation instructions. Preserve their exact files and hashes when reorganizing local evidence. They do not replace the checked-in source/dependency locks or vendored notices.
