//! Deterministic test-only pause immediately before cache publication.
#[derive(Debug, Default)]
pub(crate) struct PublicationGate {
    pub(crate) ready: tokio::sync::Notify,
    pub(crate) release: tokio::sync::Notify,
}
