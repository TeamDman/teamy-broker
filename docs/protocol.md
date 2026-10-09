# Protocol V1

Local Windows byte-mode named pipe `teamy-broker-v1-<current user SID>`. The DACL grants that user access; remote clients are rejected. First-instance creation guards startup, readiness and reconciliation against racing daemons. Each connected client uses one request and one response.

Each metadata frame has a big-endian u32 UTF-8 JSON byte length, then JSON bytes, maximum 16 KiB. Server hello is first; client replies with hello. Both advertise exact version 1 and all required capabilities. Missing capabilities or a different version are errors. The instance ID is an opaque UUID, not item data.

Request fields are `operation`, nullable `acquire`, and nullable `lease_id`. Operations are `acquire`, `validate`, `status`, `purge`, `stop`. `acquire` requires typed account, secret_ref, purpose, requesting_app, destination and exactly one backend. `purge` accepts one UUID or null for all. `validate` requires a UUID and returns only lease metadata. Other control operations accept neither acquire data nor a lease ID. Unknown operations and invalid shapes are rejected.

Response fields are `code`, nullable `lease_id`, nullable `expires_unix_ms`, nullable `remaining_millis` from the broker monotonic deadline, and nullable redacted `status`. Codes are `granted`, `ok`, `denied`, and `cleanup_failed`; clients reject unexpected codes. Only `granted` is followed by a separate big-endian u32 secret length and secret bytes, maximum 4,096. No secret value is part of a JSON frame. Lease expiration is fixed from approval and independently checked at broker dispatch and SDK use.

A handshake/read deadline bounds silent clients. The server permits at most 32 concurrent sessions, serializes exact-use approvals to avoid duplicated reads, cancels on client EOF, and services status/purge/stop independently of the approval gate. Failed requests never include raw error chains or item data in responses. This protocol does not authenticate hostile same-user code beyond the OS user ACL.
