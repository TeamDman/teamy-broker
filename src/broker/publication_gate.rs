//! Deterministic test-only pause immediately before cache publication.
use tokio::sync::Notify;
#[derive(Debug, Default)]
pub(crate) struct PublicationGate {
    pub(crate) ready: Notify,
    pub(crate) release: Notify,
}
