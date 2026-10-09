# teamy-broker

A per-user Windows credential daemon and thin Rust SDK. The broker asks for native approval of an exact item use, reads that field through the existing 1Password CLI desktop integration, and grants a fixed **15-minute hard lease starting at approval**. Activity never renews a lease. There is no secondary one-hour cap.

V1 supports Windows 10/11 desktop sessions and the MSVC Rust toolchain. It is a normal user process, not a Windows service. It requires neither elevation nor installation of a persistent startup entry. `serve` runs in the foreground; explicitly configured SDK autostart launches the same binary hidden for the current user. This repository does not install or start a production daemon as part of building or testing.

## Build and commands

```powershell
cargo build --workspace --release
.\target\release\teamy-broker.exe --help
.\target\release\teamy-broker.exe serve --op-executable 'FULL_PATH_TO_OP_EXE'
.\target\release\teamy-broker.exe status
.\target\release\teamy-broker.exe purge --lease-id OPAQUE_UUID
.\target\release\teamy-broker.exe purge --all
.\target\release\teamy-broker.exe stop
```

`status` does not autostart and shows only running state, active lease count, and cumulative cleanup failures. Purge requires exactly one of `--lease-id` or `--all`. `stop` cancels pending work, removes broker-owned leases and waits for the endpoint to disappear. Purge invalidates pending acquisitions as well as active leases. Internal `cleanup --lease-id OPAQUE_UUID` is the scheduled task entry point; it accepts no secret, item reference, or path argument.

Configure the official 1Password CLI desktop integration yourself before using real items. The default executable is `op.exe` resolved through PATH; an explicit executable path avoids PATH ambiguity. The broker invokes only `op read --account ACCOUNT --no-newline ITEM_FIELD_REFERENCE`, with stdin closed, stderr discarded, bounded secret output, a two-minute read deadline, and child termination on cancellation. Service-account, Connect and inherited session-token environment variables are removed. It never runs arbitrary caller commands, changes 1Password security settings, signs out, or performs a vault-global lock.

## SDK

Use the separate `teamy-broker-sdk` package. It builds the shared typed protocol, local IPC, protected-cache readers and lease-aware secret handle, without the broker's op adapter, native approval UI, CLI, scheduler, or tracing subscriber.

```toml
[dependencies]
teamy-broker-sdk = { git = "https://github.com/TeamDman/teamy-broker", package = "teamy-broker-sdk" }
```

```rust,no_run
use teamy_broker_sdk::{AcquireRequest, Backend, Client};
# async fn example() -> eyre::Result<()> {
let client = Client::new(Some(std::path::PathBuf::from("teamy-broker.exe")))?;
let secret = client.acquire(AcquireRequest {
    account: "your-explicit-account".into(),
    secret_ref: "op://YourVault/YourItem/YourField".into(),
    purpose: "Authenticate my application".into(),
    requesting_app: "my-application".into(),
    destination: "https://api.example.invalid".into(),
    backend: Backend::Memory,
}).await?;
secret.expose(|bytes| {
    // Hand bytes directly to the consumer operation. Do not print or log them.
    let _ = bytes;
})?;
client.purge(Some(secret.lease_id.clone())).await?;
# Ok(()) }
```

`Client::new(None)` never starts a daemon. An executable enables bounded, race-safe autostart only when the selected endpoint is absent. An existing incompatible or inaccessible broker is an error; the SDK never silently starts a replacement. The default readiness deadline is 10 seconds; acquisition has a 190-second overall deadline to cover approval and upstream authorization. Control operations can use tighter explicit deadlines through `with_deadlines`.

Consumers must select exactly one explicit provider using `resolve_provider(cli_value, environment_value)`. Both values being set is an error even if equal. Neither set, or an invalid explicit value, is an error. Valid selections are `teamy-broker`, `windows-credential-manager` and `well-known-file`. Match the selected value to the typed backend yourself; there is no discovery or fallback chain. No existing consumers were modified by this repository.

## Storage and cleanup

| Explicit backend | Storage | Cleanup |
|---|---|---|
| `Backend::Memory` | Zeroizing broker memory; default | Broker expiry/purge/shutdown; no scheduled task |
| `Backend::WindowsCredentialManager("teamy-broker/consumer/target")` | Current-user generic credential, bounded envelope | Verified per-lease Task Scheduler entry before publication |
| `Backend::WellKnownFile(absolute_path)` | Current-user DPAPI-protected envelope; atomic same-directory replacement | Verified per-lease Task Scheduler entry before publication |

Persistent locations are consumer-selected. Credential Manager targets require the `teamy-broker/` namespace. File parents must already exist. Existing destinations that do not contain a recognized broker envelope are rejected. File aliases share a canonical-parent lock key; generation IDs ensure an old cleanup cannot remove a replacement. Credential Manager's 2,560-byte blob limit permits at most 2,528 secret bytes; memory and file backends permit 4,096. The file backend uses Windows DPAPI, not custom cryptography.

Persistent publication writes durable opaque-lease metadata, registers and verifies a least-privilege current-user Task Scheduler 2.0 task with `StartWhenAvailable`, then publishes secret bytes. The task command contains only the cleanup verb and an opaque lease UUID. No secrets, item references or destinations are in task arguments. Cleanup locks the destination and deletes only its matching generation. Non-secret lock files are retained to keep the lock identity stable across concurrent processes. Failed cleanup is retained for retry, and broker-visible secret bytes are cleared immediately. Purge reports incomplete cleanup. Persistent backend registration failures prevent a grant.

`load_cached(backend, lease_id)` reads only the selected persistent provider. It validates the live broker lease before and after the read, checks envelope generation and exact expiry, and never autostarts, approves a new item or falls back. The SDK and broker reject expired leases even if the machine was asleep or cleanup was delayed. Broker restart does not resurrect old leases: startup reconciles its old descriptors before serving requests. If reconciliation fails, startup fails closed.

Task Scheduler can run late after resume. Removing or relocating the executable, logging out, disabling Task Scheduler, process crashes, disk failures, and permission changes can delay physical deletion. The scheduler uses three short retries and a long end boundary for missed triggers. Successful SDK validation requires the original live lease; physical bytes remaining on disk are not valid leases. This is not a claim that the OS will delete every copy precisely at 15 minutes.

## Approval and security boundary

The native TaskDialog shows the exact account, item field reference, purpose, claimed application, connected process ID, destination, storage destination and fixed lease duration. Denial is the default; closing, timing out at 60 seconds, disconnecting or cancelling denies. The connected PID comes from the Windows named pipe API. Application names and intent are caller-supplied claims.

This is a **cooperative same-user boundary**, not malware isolation. The named pipe rejects remote clients and has an explicit current-user DACL. A same-user program can spoof the UI, impersonate claims, read or modify that user's state, copy authorized bytes, call op directly, or inspect processes. Administrators and compromised OS components are outside the boundary. UI spoofing risk is explicitly accepted. There is no secure-desktop attestation or binary-signature identity proof.

Only the broker invoking op is a consumer convention. 1Password's own authorization remains account/session based, with 10 minutes idle expiry and a separate 12-hour maximum. Keeping a broker context alive does not make that authorization item-specific. The broker adds an item-specific preflight for each new exact use; cached identical use shares the existing fixed lease. Upstream reauthorization may still be required. See [1Password's authorization model](https://developer.1password.com/docs/cli/app-integration-security#authorization-model).

Purge and stop cannot revoke caller copies, reverse an API call, revoke upstream tokens, or guarantee global vault locking. SDK handles conservatively bound the broker monotonic remainder by local request duration, so transfer delays and wall-clock rollback cannot extend a lease; a handle can expire earlier than the broker lease. Secret handles do not implement reflection/JSON serialization and redact Debug; bytes can be obtained only through an explicit closure after wall-clock and monotonic checks. A caller can intentionally copy them. Zeroization reduces lingering application buffers but cannot guarantee erasure from paging, dumps, OS credential internals or compiler/runtime copies.

## Diagnostics and verification

Stdout contains redacted command results only. Diagnostics use stderr and optional explicit log files, with independent filters and text/NDJSON encoding inherited from the Teamy CLI template. Producer code never logs secrets, item metadata, raw arguments or arbitrary error/panic payloads. See [logging contract](docs/logging-progress.md), [protocol](docs/protocol.md), and [threat model](docs/threat-model.md).

```powershell
cargo +nightly fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --no-deps -- -D warnings
cargo build --examples
cargo test --workspace --features extended_observability
.\check-lint-policy.ps1
cargo run --example synthetic_consumer -- --provider teamy-broker
```

The default tests use fake adapters, injected approvals and storage plus isolated real named pipes. The synthetic autostart helper has a 30-second safety deadline and is stopped by the test. It never invokes op or writes real credentials. Native OS backend tests are ignored by default; only run them after reviewing the isolated synthetic writes and temporary task registration:

```powershell
cargo test --test native_backend -- --ignored --test-threads=1
```

Those tests use random `teamy-broker/synthetic-test/` targets, temporary DPAPI files and exact opaque task IDs, and verify their removal. No real vault reads, real approval acceptance, OAuth grants, security-setting changes or live API calls are part of validation. Native UI and actual 1Password authorization require separate human testing; synthetic tests do not claim to verify those live flows.

MPL-2.0. Started from the verified Teamy Rust CLI source snapshot based on `99460d927c745a43285b0c5206fc2d42cec33ebc`; changes are confined to this new repository.
