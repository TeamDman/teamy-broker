# Broker-owned shutdown status

`status` is a nonsecret snapshot and never waits on the approval/acquisition gate.
It does not perform cleanup. The regular maintenance loop owns expiry and retries.

- `active_requests` includes acquisitions through grant handoff and purge/validation controls; status and stop do not count themselves.
- `queued_requests` counts acquisitions waiting for the serialization gate. They may reuse an existing lease, so this is not a count of promised dialogs.
- `active_approvals` includes queued blocking approval jobs, live dialogs, and final native teardown. `pending_approvals` is the subset without an open broker dialog.
- `open_approval_dialogs` tracks the broker's native TaskDialog CREATED/DESTROYED notifications and RAII teardown. Cancellation of an async caller cannot hide a still-open blocking dialog.
- `upstream_reads` tracks the credential adapter/op read operation. It says nothing about visibility of external 1Password UI.
- `storage_operations` tracks publishing, retirement, expiry and cleanup work. `pending_cleanup` counts failed removals awaiting retry; `cleanup_failures` remains the historical failure counter.
- `would_interrupt_operations` and `would_discard_cached_leases` distinguish interruption from discarding idle cached leases.
- `safe_to_stop` requires no current work, no live cached leases, no pending cleanup, and no shutdown already underway. `stop_reasons` contains `active_operations`, `cached_leases`, `pending_cleanup`, or `already_stopping` as appropriate.

`stop` rechecks work under the admission lock and current lease/cleanup state,
then closes admission before cleanup. A stale safe snapshot cannot authorize a
newly busy daemon's shutdown. A refused stop leaves admission and existing
leases intact. `stop --force` explicitly authorizes maintenance interruption and
lease discard; it never persists or migrates plaintext leases. Cleanup failure
still fails closed. Internal cancellation retains the existing authorized
graceful shutdown behavior.

Protocol version 2 and its required acquisition capabilities remain unchanged.
Updated servers advertise `shutdown-state-v1` and `stop-force`; updated clients
check the shutdown capability before sending either stop operation. Old status
responses deserialize with `shutdown_state_available=false`, `safe_to_stop=false`
and client reason `status_unavailable`. New counters on such responses are default
placeholders, not observations. Update the daemon explicitly to obtain shutdown
state. Existing acquisition compatibility checks and refusal to autostart an
incompatible peer are unchanged. Older clients may still acquire from the new
daemon, but their unqualified stop is now guarded and may be refused.

Tests use isolated `teamy-broker-test-*` pipes and synthetic adapters only.
