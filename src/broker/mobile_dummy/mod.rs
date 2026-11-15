//! Explicit synthetic adapter. Never registered by the production `serve` command.
use crate::protocol::AcquireRequest;
use crate::protocol::Backend;
pub mod adapter;
pub mod approval;
pub mod bootstrap;
pub mod broadcast_approval;
mod desktop_prompt;
mod desktop_prompt_state;
mod java_executable_path;
pub mod packet;
pub use java_executable_path::JavaExecutablePath;
pub mod phone_profile;
mod phone_profile_migration;
pub use phone_profile_migration::PhoneProfileMigration;
mod pending_guard;
mod phone_profile_preflight;
mod phone_profile_v0;
mod phone_profile_v1;
pub mod server;
pub mod storage;
pub mod tailnet_peer;
mod tailscale_tailnet_node_id;
pub use tailscale_tailnet_node_id::TailscaleTailnetNodeId;
mod tailscale_tailnet_ipv4_addr;
pub use tailscale_tailnet_ipv4_addr::TailscaleTailnetIpv4Addr;
mod tailscale_tailnet_node_name;
pub use tailscale_tailnet_node_name::TailscaleTailnetNodeName;
mod tailscale_tailnet_user_id;
pub use tailscale_tailnet_user_id::TailscaleTailnetUserId;
mod tailscale_host;
mod tailscale_node;
mod tailscale_self;
mod tailscale_status;
mod tailscale_whois;
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
