use crate::broker::phone::config::PhoneProfile;
use crate::paths::AppHome;
use facet::Facet;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::process::Command;
use tokio::time;
pub const PORT: u16 = 43188;
/// Restricts listeners and live peer identity checks to the configured phone relationship.
#[derive(Facet, Debug, Clone, Copy)]
pub struct TailnetPeer;
impl TailnetPeer {
    #[must_use]
    pub fn permits_bind_for(address: SocketAddr, profile: &PhoneProfile) -> bool {
        address.ip() == IpAddr::V4(profile.desktop.get()) && address.port() == PORT
    }
    #[must_use]
    pub fn permits_source_for(source: SocketAddr, profile: &PhoneProfile) -> bool {
        source.ip() == IpAddr::V4(profile.phone.get())
    }
    /// # Errors
    /// Rejects unavailable, changed or mismatched desktop identity.
    pub async fn verify_desktop_for(profile: &PhoneProfile) -> eyre::Result<()> {
        Self::require_current(profile)?;
        let json = read_cli(&["status", "--json"]).await?;
        let status: super::tailscale_status::TailscaleStatus = facet_json::from_str(&json)
            .map_err(|_redacted_error| eyre::eyre!("tailnet state unavailable"))?;
        eyre::ensure!(
            status.state == "Running"
                && status.node.id.as_str() == profile.desktop_node.as_str()
                && status.node.os == "windows"
                && status.node.user_id == profile.user.get()
                && status.node.ips.contains(&profile.desktop.get().to_string()),
            "tailnet desktop rejected"
        );
        Self::require_current(profile)
    }
    /// # Errors
    /// Rejects a changed profile or a mismatched source and Tailscale identity.
    pub async fn verify_peer_for(source: SocketAddr, profile: &PhoneProfile) -> eyre::Result<()> {
        Self::require_current(profile)?;
        eyre::ensure!(
            source.ip() == IpAddr::V4(profile.phone.get()),
            "tailnet peer rejected"
        );
        let address = source.to_string();
        let json = read_cli(&["whois", "--json", &address]).await?;
        Self::validate_whois(&json, profile)?;
        Self::require_current(profile)
    }
    /// Validate a read-only CLI result separately so negative identity cases need no live node changes.
    /// # Errors
    /// Rejects missing/malformed or mismatched identity information.
    pub fn validate_whois(json: &str, profile: &PhoneProfile) -> eyre::Result<()> {
        let result: super::tailscale_whois::TailscaleWhois = facet_json::from_str(json)
            .map_err(|_redacted_error| eyre::eyre!("tailnet peer unavailable"))?;
        let node = result.node;
        let expiry = chrono::DateTime::parse_from_rfc3339(&node.key_expiry)
            .map_err(|_redacted_error| eyre::eyre!("tailnet peer unavailable"))?;
        eyre::ensure!(
            node.stable_id.as_str() == profile.phone_node.as_str()
                && node.user == profile.user.get()
                && node.host.os == "android"
                && node.name.as_str() == profile.phone_name.as_str()
                && node
                    .addresses
                    .contains(&format!("{}/32", profile.phone.get()))
                && expiry > chrono::Utc::now(),
            "tailnet peer rejected"
        );
        Ok(())
    }
    /// # Errors
    /// Rejects missing, invalid, or changed profile data.
    pub fn require_current(profile: &PhoneProfile) -> eyre::Result<()> {
        profile.require_current(&AppHome::resolve()?)
    }
}
async fn read_cli(args: &[&str]) -> eyre::Result<String> {
    let mut command = Command::new(r"C:\Program Files\Tailscale\tailscale.exe");
    command.args(args).kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW; existing user token, no elevation.
    let output = time::timeout(Duration::from_secs(3), command.output())
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
