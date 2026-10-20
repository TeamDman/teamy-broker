/// Trusted local dummy UI decisions. Dismissal is deliberately not a decision.
#[derive(Debug, Clone, Copy)]
pub enum LocalDecision {
    SupplyDummy,
    Deny,
    Cancel,
}
