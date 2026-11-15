//! Synthetic handoff adapters and loopback fixtures; never registered by `serve`.
use crate::protocol::AcquireRequest;
use crate::protocol::Backend;
pub mod adapter;
pub mod approval;
mod pending_guard;
pub mod server;
pub mod storage;
pub mod tls_identity;
pub mod tls_server;
#[must_use]
pub fn request() -> AcquireRequest {
    AcquireRequest {
        account: "synthetic-account".into(),
        secret_ref: teamy_broker_handoff::REFERENCE.into(),
        purpose: teamy_broker_handoff::PURPOSE.into(),
        requesting_app: teamy_broker_handoff::CONSUMER.into(),
        destination: "https://example.invalid".into(),
        backend: Backend::Memory,
    }
}
