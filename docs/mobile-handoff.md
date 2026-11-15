# Mobile handoff and pairing ownership

`teamy-broker-handoff` contains the experimental synthetic handoff protocol and shared pairing records. Its package description predates persistent phone support. Some consumers are test fixtures; others are part of the resident phone implementation. Use the consumer map below before removing an export or changing its wire representation.

| Component | Responsibility and consumers |
| --- | --- |
| `handoff/src/{binding,envelope,pending,queue,session,wire}.rs` | Synthetic request binding, AES-GCM envelopes, bounded in-memory queue and framed transport. `examples/mobile_handoff_*.rs` and the mobile contract tests exercise these paths. |
| `handoff/src/{clock,clock_reading,deadline}.rs` | Wall and monotonic deadline behavior used by the synthetic queue and its race tests. |
| `handoff/src/pairing_binding.rs` | Canonical application context authenticated by both UKEY2 peers. Its `dummy` constructor supplies synthetic defaults; the record is also used by persistent enrollment. |
| `src/broker/mobile_dummy/` | Mixed legacy module: synthetic adapters coexist with the Java pairing worker, comparison UI, runtime phone profile, peer checks and persistent enrollment used by the resident. The directory name is not an isolation boundary. |
| `src/broker/phone/` | Resident relationship state, protected persistence, revocation, TLS connections and request authorization. See [phone approvals](phone-approvals.md). |
| `pairing/java/` and `android/` | Pinned UKEY2 engine plus its private desktop worker, and the native Android application. Android's current package directory retains the historical `dummy` name; build identity and runtime admission determine its capabilities. |

## Synthetic handoff contract

The legacy handoff fixture accepts only the fixed `DUMMY_SECRET` and exact synthetic metadata in `handoff/src/lib.rs`. Its queue binds the request identity, nonce, account, consumer, destination, scope and expiry. Desktop and phone decisions compete for one entry; the first valid decision consumes it. Deadline checks occur before delivery as well as before consumption. Queue acknowledgment records acceptance of a decision, not successful SDK grant publication.

Synthetic acquisition reuses the broker's approval, cancellation, purge and fixed 15-minute memory-lease behavior. A later cancellation, purge or expiry can prevent publication. Purging broker state cannot revoke bytes a consumer already copied. The separate resident credential fixture uses `src/broker/synthetic_credential_fixture.rs` and `Client::acquire_fixture`; these fixtures are different contracts. A vault name or reference prefix never identifies a safe fixture.

## Pairing implementation and provenance

The current desktop worker and Android app use the official Google UKEY2 Java implementation pinned by `pairing/java/vendor-lock.json`, with archive and per-file digests. Keep the vendored `LICENSE` and `NOTICE` with that source. `pairing/java/dependencies-lock.json` records exact external artifacts and their hashes; `pairing/java/build.ps1` verifies them without downloading or running upstream Gradle scripts. Android packaging includes the UKEY2 license and notice under APK assets. Hash agreement is reproducibility evidence, not an independent security audit or a complete third-party license audit.

Both peers must confirm the same live comparison before application traffic is accepted. Role and exact application binding are authenticated inside the derived connection context. `VerifiedPairing` gives transport callers cloned handshake buffers: the official engine retains transcript arrays until key derivation. Clearing a transport buffer must not clear the engine's transcript. Independent official-peer tests in `pairing/java` cover each wrapped role; keep that regression when replacing the worker.

The Windows runtime currently requires Java artifacts in the source-tree layout and a configured absolute Java executable. A Rust Android migration must preserve pairing interoperability, native input/lifecycle behavior and platform key storage before replacing this integration. Makepad is a reference under investigation, not a selected dependency.

## Build and validation

Use the offline commands and environment prerequisites in [phone approvals](phone-approvals.md#dependencies-and-offline-checks). [Validation scope](validation.md) distinguishes host tests, packaged artifacts and device acceptance. Building an APK or passing a JVM exchange does not establish Android hardware, Autofill, notification or sleep behavior.

SDK installer receipts and obsolete APK/runtime checkpoints are local historical evidence, not application source or current installation instructions. Preserve their exact files and hashes when reorganizing local evidence. They do not replace the checked-in source/dependency locks or vendored notices.
