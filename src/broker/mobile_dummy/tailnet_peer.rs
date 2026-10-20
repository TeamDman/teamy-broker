use facet::Facet;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::time::Duration;
use teamy_broker_handoff::phone_config::CONFIGURED;
pub use teamy_broker_handoff::phone_config::DESKTOP;
pub use teamy_broker_handoff::phone_config::DESKTOP_NODE;
pub use teamy_broker_handoff::phone_config::PHONE;
use teamy_broker_handoff::phone_config::PHONE_NAME;
pub use teamy_broker_handoff::phone_config::PHONE_NODE;
pub use teamy_broker_handoff::phone_config::PORT;
use teamy_broker_handoff::phone_config::USER;
/// Inspected synthetic-test scope. No arbitrary hosts, all-interface binds or persisted pairing.
#[derive(Facet, Debug, Clone, Copy)]
pub struct TailnetPeer;
/// Explicit private build configuration is required before any non-loopback listener can start.
pub const ACTIVATION_APPROVED: bool = CONFIGURED;
impl TailnetPeer {
    #[must_use]
    pub fn permits_bind(address: SocketAddr) -> bool {
        address.ip() == IpAddr::V4(DESKTOP) && address.port() == PORT
    }
    #[must_use]
    pub fn permits_source(source: SocketAddr) -> bool {
        source.ip() == IpAddr::V4(PHONE)
    }
    /// # Errors
    /// Fails closed if the local Tailscale state is unavailable or differs from the inspected scope.
    pub async fn verify_desktop() -> eyre::Result<()> {
        eyre::ensure!(CONFIGURED, "private phone configuration unavailable");
        let json = read_cli(&["status", "--json"]).await?;
        let status: super::tailscale_status::TailscaleStatus = facet_json::from_str(&json)
            .map_err(|_redacted_error| eyre::eyre!("tailnet state unavailable"))?;
        eyre::ensure!(
            status.state == "Running"
                && status.node.id == DESKTOP_NODE
                && status.node.os == "windows"
                && status.node.user_id == USER
                && status.node.ips.contains(&DESKTOP.to_string()),
            "tailnet desktop rejected"
        );
        Ok(())
    }
    /// # Errors
    /// Rejects other addresses, nodes, users, operating systems or expired node credentials.
    pub async fn verify_peer(source: SocketAddr) -> eyre::Result<()> {
        eyre::ensure!(Self::permits_source(source), "tailnet peer rejected");
        let address = source.to_string();
        let json = read_cli(&["whois", "--json", &address]).await?;
        Self::validate_whois(&json)
    }
    /// Validate a read-only CLI result separately so negative identity cases need no live node changes.
    /// # Errors
    /// Rejects missing/malformed or mismatched identity information.
    pub fn validate_whois(json: &str) -> eyre::Result<()> {
        let result: super::tailscale_whois::TailscaleWhois = facet_json::from_str(json)
            .map_err(|_redacted_error| eyre::eyre!("tailnet peer unavailable"))?;
        let node = result.node;
        let expiry = chrono::DateTime::parse_from_rfc3339(&node.key_expiry)
            .map_err(|_redacted_error| eyre::eyre!("tailnet peer unavailable"))?;
        eyre::ensure!(
            node.stable_id == PHONE_NODE
                && node.user == USER
                && node.host.os == "android"
                && node.name == PHONE_NAME
                && node.addresses.contains(&format!("{PHONE}/32"))
                && expiry > chrono::Utc::now(),
            "tailnet peer rejected"
        );
        Ok(())
    }
}
async fn read_cli(args: &[&str]) -> eyre::Result<String> {
    let mut command = tokio::process::Command::new(r"C:\Program Files\Tailscale\tailscale.exe");
    command.args(args).kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW; existing user token, no elevation.
    let output = tokio::time::timeout(Duration::from_secs(3), command.output())
        .await
        .map_err(|_redacted_error| eyre::eyre!("tailnet inspection timed out"))?
        .map_err(|_redacted_error| eyre::eyre!("tailnet inspection unavailable"))?;
    eyre::ensure!(
        output.status.success() && output.stdout.len() <= 262_144,
        "tailnet inspection unavailable"
    );
    String::from_utf8(output.stdout)
        .map_err(|_redacted_error| eyre::eyre!("tailnet inspection unavailable"))
}
