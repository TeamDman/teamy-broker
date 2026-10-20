# Protocol V2

Local Windows byte-mode named pipe `teamy-broker-v1-<current user SID>`. The DACL grants that user access; remote clients are rejected. First-instance creation guards startup, readiness and reconciliation against racing daemons. Each connected client uses one request and one response.

Each metadata frame has a big-endian u32 UTF-8 JSON byte length, then JSON bytes, maximum 16 KiB. Server hello is first; client replies with hello. Both advertise exact version 2 and all required capabilities. Missing capabilities or a different version are errors. The instance ID is an opaque UUID, not item data.

Request fields are `operation`, nullable `acquire`, and nullable `lease_id`. Core operations are `acquire`, `validate`, `status`, `purge`, `stop`, `stop-force`. `acquire` requires typed account, secret_ref, purpose, requesting_app, destination and exactly one backend. `purge` accepts one UUID or null for all. `validate` requires a UUID and returns only lease metadata. Other control operations accept neither acquire data nor a lease ID. Unknown operations and invalid shapes are rejected.

Response fields are `code`, nullable typed `failure`, nullable `lease_id`, nullable `expires_unix_ms`, nullable `remaining_millis` from the broker monotonic deadline, and nullable redacted `status`. Codes are `granted`, `ok`, `failed`, `denied` (control lease validation), and `cleanup_failed`; clients reject unexpected codes. Only `granted` is followed by a separate big-endian u32 secret length and secret bytes, maximum 4,096. No credential value is part of a JSON frame. Lease expiration is fixed from approval and independently checked at broker dispatch and SDK use.

A handshake/read deadline bounds silent clients. The server permits at most 32 concurrent sessions, serializes exact-use approvals to avoid duplicated reads, cancels on client EOF, and services status/purge/stop independently of the approval gate. Failed requests never include raw error chains or item data in responses. This protocol does not authenticate hostile same-user code beyond the OS user ACL.

The discovery endpoint deliberately retains its v1 name. Protocol v2 requires `typed-acquisition-errors-v1`; v1/missing-capability peers are rejected and never replaced through silent autostart. Build the broker and SDK from a matching reviewed v2 revision; the original v1 SDK is incompatible.

Acquisition failures carry only the closed `FailureKind` enum, without item metadata, raw upstream details or secret bytes. `UserDenied` denotes the broker preflight's explicit denial/close, `ApprovalTimedOut` its hard one-minute deadline, and `UpstreamReadFailed` any failure in the fixed op-read adapter (including 1Password refusal). The SDK preserves these as typed downcastable errors with static messages; unexpected/malformed reply payloads produce `RequestFailed` without parser payloads. No secret frame follows `failed`. Other safe variants describe cancellation, unavailable UI, expired lease, failed cache publication and overall request timeout.

Windows autostart uses CreateProcessW with handle inheritance disabled and no console. A detached daemon cannot retain a consumer's redirected output or unrelated inheritable handles. Launch quoting follows Windows CRT argument rules; no shell is involved.

Optional `shutdown-state-v1` and `stop-force` capabilities extend protocol v2 without changing acquisition compatibility. SDK shutdown requires the advertised safety capability before sending a request. Legacy status remains readable but its unavailable work state is explicitly unsafe to infer; see [shutdown status](shutdown-status.md).

Optional capability-negotiated phone, pairing-comparison and synthetic-fixture operations are described in [phone approvals](phone-approvals.md). Pairing inspection deliberately returns only the exact active attempt's ephemeral comparison; it grants no credential access.
