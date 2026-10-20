use super::stop_reason::StopReason;
use facet::Facet;

/// Nonsecret snapshot. Stop rechecks admission and work rather than trusting this receipt.
#[derive(Facet, Debug, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Wire snapshots expose independent compatibility and shutdown facts, not mutable state-machine flags"
)]
pub struct Status {
    pub running: bool,
    pub active_leases: u64,
    pub cleanup_failures: u64,
    #[facet(default)]
    pub shutdown_state_available: bool,
    #[facet(default)]
    pub active_requests: u64,
    #[facet(default)]
    pub queued_requests: u64,
    #[facet(default)]
    pub active_approvals: u64,
    #[facet(default)]
    pub pending_approvals: u64,
    #[facet(default)]
    pub open_approval_dialogs: u64,
    #[facet(default)]
    pub upstream_reads: u64,
    #[facet(default)]
    pub storage_operations: u64,
    #[facet(default)]
    pub pending_cleanup: u64,
    #[facet(default)]
    pub stopping: bool,
    #[facet(default)]
    pub would_interrupt_operations: bool,
    #[facet(default)]
    pub would_discard_cached_leases: bool,
    #[facet(default)]
    pub safe_to_stop: bool,
    #[facet(default)]
    pub stop_reasons: Vec<StopReason>,
}
impl Status {
    #[must_use]
    pub fn stopped() -> Self {
        Self {
            safe_to_stop: true,
            shutdown_state_available: true,
            ..Self::default()
        }
    }
}
