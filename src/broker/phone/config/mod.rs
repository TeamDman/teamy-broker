//! Runtime phone profile, schema migration and validated configuration values.

mod java_executable_path;
pub mod phone_config_action;
pub mod phone_config_error;
pub mod phone_config_report;
pub mod phone_config_request;
pub mod phone_config_values;
pub mod phone_profile;
mod phone_profile_migration;
mod phone_profile_preflight;
mod phone_profile_v0;
mod phone_profile_v1;
mod tailscale_tailnet_ipv4_addr;
mod tailscale_tailnet_node_id;
mod tailscale_tailnet_node_name;
mod tailscale_tailnet_user_id;

pub use java_executable_path::JavaExecutablePath;
pub use phone_profile::PhoneProfile;
pub use phone_profile_migration::PhoneProfileMigration;
pub use tailscale_tailnet_ipv4_addr::TailscaleTailnetIpv4Addr;
pub use tailscale_tailnet_node_id::TailscaleTailnetNodeId;
pub use tailscale_tailnet_node_name::TailscaleTailnetNodeName;
pub use tailscale_tailnet_user_id::TailscaleTailnetUserId;
