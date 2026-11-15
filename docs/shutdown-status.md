# Broker-owned shutdown status

`status` is a nonsecret snapshot. It reads brief state locks without waiting on
the approval/acquisition gate or performing native storage operations. The
regular maintenance loop owns expiry and cleanup retries.

- `active_requests` includes acquisitions through grant handoff and purge/validation controls; status and stop do not count themselves.
- `queued_requests` counts acquisitions waiting for the serialization gate. They may reuse an existing lease, so this is not a count of promised dialogs.
- `active_approvals` includes queued blocking approval jobs, live dialogs, and final native teardown. `pending_approvals` is the subset without an open broker dialog.
- `open_approval_dialogs` tracks the broker's native TaskDialog CREATED/DESTROYED notifications and RAII teardown. Cancellation of an async caller cannot hide a still-open blocking dialog.
- `upstream_reads` tracks the credential adapter/op read operation. It says nothing about visibility of external 1Password UI.
- `storage_operations` tracks publishing, retirement, expiry and cleanup work, including blocking workers whose async callers have been cancelled. The count can include both an operation and its cleanup worker. `pending_cleanup` counts failed removals awaiting retry; `cleanup_failures` remains the historical failure counter.
- `would_interrupt_operations` and `would_discard_cached_leases` distinguish interruption from discarding idle cached leases.
- `safe_to_stop` requires no current work, no live cached leases, no pending cleanup, and no shutdown already underway. `stop_reasons` contains `active_operations`, `cached_leases`, `pending_cleanup`, or `already_stopping` as appropriate.

Native publication and removal run on tracked blocking workers outside the
in-memory lease and authorization locks. Publication retains ownership
through grant handoff or rollback. If cancellation, revocation, expiry or
`purge --all` invalidates a request while a write is running, the write may finish physically,
but the worker rejects the grant and cleans up only that lease generation.
Cancelling the async caller does not cancel or hide that worker.

Purge and expiry remove leases from the live map and schedule their whole cleanup
batch before releasing the map lock. Retry does the same when taking failed
removals from the pending list. A cancelled purge or maintenance future therefore
cannot discard cleanup work between removing the records and starting a worker.
Native removal runs after that transfer without holding the map lock.

`purge --lease-id` retires the matching live lease. `purge --all` also invalidates
acquisitions already pending when it begins and waits for tracked publication,
rollback and retirement workers. Both forms retry failed cleanup and report an
error if cleanup remains pending. Purge leaves admission open for new requests.

`stop` rechecks work under the admission lock and current lease/cleanup state,
then closes admission before cleanup. A stale safe snapshot cannot authorize a
newly busy daemon's shutdown. A refused ordinary stop leaves admission and
existing leases intact. `stop --force` closes admission, invalidates pending
acquisitions and drains tracked storage workers before reporting success. It
never persists or migrates plaintext leases. Server teardown also drains storage
after cancelling its async client and maintenance tasks, keeping the listener
owned until that cleanup attempt finishes.

A failed removal keeps a retry record with its in-memory secret bytes cleared;
the removed lease remains unusable. Persistent cleanup metadata remains available
for retry, and generation checks preserve a newer replacement. Cleanup failure
is reported rather than treated as successful shutdown. After an admitted stop
fails cleanup, admission stays closed; maintenance can retry, and a later forced
stop can complete shutdown. Physical deletion can still be delayed by OS or disk
failures. Purge and stop cannot revoke copies already returned to a consumer.

Protocol version 2 and its required acquisition capabilities remain unchanged.
Updated servers advertise `shutdown-state-v1` and `stop-force`; updated clients
check the shutdown capability before sending either stop operation. Old status
responses deserialize with `shutdown_state_available=false`, `safe_to_stop=false`
and client reason `status_unavailable`. New counters on such responses are default
placeholders, not observations. Update the daemon explicitly to obtain shutdown
state. Existing acquisition compatibility checks and refusal to autostart an
incompatible peer are unchanged. Older clients may still acquire from the new
daemon, but their unqualified stop is now guarded and may be refused.

Shutdown tests use isolated `teamy-broker-test-*` pipes and synthetic adapters.
Injected blocking storage tests cover responsive status, cancelled publication
and retirement, unreceived grants, exact-generation rollback and worker draining.
They do not qualify live Credential Manager, Task Scheduler or device behavior.
