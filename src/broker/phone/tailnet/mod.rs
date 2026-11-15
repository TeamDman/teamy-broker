//! Exact configured peer checks against the current Tailscale state.

mod tailnet_peer;
mod tailscale_host;
mod tailscale_node;
mod tailscale_self;
mod tailscale_status;
mod tailscale_whois;

pub use tailnet_peer::PORT;
pub use tailnet_peer::TailnetPeer;
