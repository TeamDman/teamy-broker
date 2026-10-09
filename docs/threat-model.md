# Threat model

The trusted actors are a cooperating user, their selected consumer and broker, Windows APIs, and the configured 1Password CLI/desktop integration. The broker enforces exact-use preflight and time-bounded cooperative access, avoiding routine repeated upstream reads.

Assets are returned credential bytes, approval intent, lease deadlines, and generation-specific cleanup ownership. IPC metadata and status omit secret values; status also omits accounts, item references, paths and purposes. Secret bytes use bounded binary framing and zeroizing buffers, not reflected JSON models.

Same-user hostile code, spoofed UI, false claimed app names/destinations, administrator access, OS compromise, intentionally retained copies, external op invocations, upstream authorization revocation and paging/dumps are outside the guarantees. A named-pipe user ACL is useful access control but is not a defense against that same user's malware. Approval is not a secure-desktop confirmation.

Fail-closed cases include denied/timed-out/cancelled approval, disconnected consumers, invalid request shape, incompatible IPC, failed cleanup registration, stale storage generations, expired monotonic or wall-clock deadlines, and broker restart. Persistent tasks support late resume cleanup; logical expiration is enforced independently of task timing. A failed purge may leave physically stored encrypted bytes and a retry descriptor, while denying the removed lease.

Review changes to native UI, pipe security, op arguments/environment, secret framing, persistence publication order, locking, task arguments and SDK deadlines together. Test with synthetic values only. A new operation must not become an arbitrary command proxy or a silent provider fallback.
