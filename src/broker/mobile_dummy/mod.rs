//! Explicit synthetic adapter. Never registered by the production `serve` command.
pub mod adapter;
pub mod approval;
pub mod bootstrap;
pub mod broadcast_approval;
mod desktop_prompt;
mod desktop_prompt_state;
pub mod packet;
mod pending_guard;
pub mod server;
pub mod storage;
pub mod tailnet_peer;
mod tailscale_host;
mod tailscale_node;
mod tailscale_self;
mod tailscale_status;
mod tailscale_whois;
pub mod tls_identity;
pub mod tls_server;
#[must_use]
pub fn request() -> crate::protocol::AcquireRequest {
    crate::protocol::AcquireRequest {
        account: "synthetic-account".into(),
        secret_ref: teamy_broker_handoff::REFERENCE.into(),
        purpose: teamy_broker_handoff::PURPOSE.into(),
        requesting_app: teamy_broker_handoff::CONSUMER.into(),
        destination: "https://example.invalid".into(),
        backend: crate::protocol::Backend::Memory,
    }
}

pub mod java_pairing_worker;

mod paired_preview;
pub mod paired_socket;
mod pairing_prompt;
mod pairing_prompt_context;
mod pairing_prompt_diagnostics;
pub mod pairing_prompt_probe;
pub mod pairing_prompt_probe_report;
mod pairing_prompt_state;
pub mod pairing_server;
mod persistent_enrollment;
pub mod preview_activation;
pub mod preview_phase;

#[cfg(test)]
mod java_pairing_worker_tests;
#[cfg(test)]
mod paired_socket_tests;

#[cfg(test)]
mod paired_sdk_tests;
