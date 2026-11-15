use crate::broker::phone::config::JavaExecutablePath;
use crate::broker::phone::config::PhoneProfile;
use crate::broker::phone::config::TailscaleTailnetNodeId;
use crate::broker::phone::config::TailscaleTailnetNodeName;
use crate::broker::phone::config::TailscaleTailnetUserId;
use crate::broker::phone::config::phone_config_action::PhoneConfigAction;
use crate::broker::phone::config::phone_config_request::PhoneConfigRequest;
use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;
use std::fmt;
use std::path::PathBuf;

#[derive(Facet, Arbitrary, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct PhoneConfigureArgs {
    /// Desktop Tailscale IPv4 address, within 100.64.0.0/10.
    #[facet(args::named)]
    pub desktop_ip: String,
    /// Phone Tailscale IPv4 address, distinct from the desktop.
    #[facet(args::named)]
    pub phone_ip: String,
    /// Exact stable desktop node ID from Tailscale status.
    #[facet(args::named)]
    pub desktop_node: String,
    /// Exact stable phone node ID from Tailscale whois.
    #[facet(args::named)]
    pub phone_node: String,
    /// Nonzero Tailscale user ID shared by both nodes.
    #[facet(args::named)]
    pub tailnet_user: u64,
    /// Exact phone DNS name from Tailscale whois.
    #[facet(args::named)]
    pub phone_name: String,
    /// Absolute path to the installed Java executable used for enrollment.
    #[facet(args::named)]
    pub java_executable: String,
    /// Explicit absolute configuration directory; defaults to the application home.
    #[facet(args::named)]
    pub app_home: Option<String>,
}
impl fmt::Debug for PhoneConfigureArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PhoneConfigureArgs([redacted])")
    }
}
impl PhoneConfigureArgs {
    /// # Errors
    /// Rejects invalid peer values, profile combinations, or relative paths.
    pub fn into_request(self) -> eyre::Result<PhoneConfigRequest> {
        let profile = PhoneProfile {
            desktop: self.desktop_ip.parse()?,
            phone: self.phone_ip.parse()?,
            desktop_node: TailscaleTailnetNodeId::try_from(self.desktop_node)?,
            phone_node: TailscaleTailnetNodeId::try_from(self.phone_node)?,
            user: TailscaleTailnetUserId::try_from(self.tailnet_user)?,
            phone_name: TailscaleTailnetNodeName::try_from(self.phone_name)?,
            java: JavaExecutablePath::try_from(self.java_executable)?,
        };
        PhoneConfigRequest::new(
            self.app_home.map(PathBuf::from),
            PhoneConfigAction::Configure(profile),
        )
    }
}
