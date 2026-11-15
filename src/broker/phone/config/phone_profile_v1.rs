use super::JavaExecutablePath;
use super::TailscaleTailnetIpv4Addr;
use super::TailscaleTailnetNodeId;
use super::TailscaleTailnetNodeName;
use super::TailscaleTailnetUserId;
use super::phone_profile::PhoneProfile;
use facet::Facet;

#[derive(Debug, Facet)]
pub(super) struct PhoneProfileV1 {
    pub schema_version: u32,
    #[facet(rename = "TEAMY_BROKER_DESKTOP_IP")]
    pub desktop: TailscaleTailnetIpv4Addr,
    #[facet(rename = "TEAMY_BROKER_PHONE_IP")]
    pub phone: TailscaleTailnetIpv4Addr,
    #[facet(rename = "TEAMY_BROKER_DESKTOP_NODE")]
    pub desktop_node: TailscaleTailnetNodeId,
    #[facet(rename = "TEAMY_BROKER_PHONE_NODE")]
    pub phone_node: TailscaleTailnetNodeId,
    #[facet(rename = "TEAMY_BROKER_TAILNET_USER")]
    pub user: TailscaleTailnetUserId,
    #[facet(rename = "TEAMY_BROKER_PHONE_NAME")]
    pub phone_name: TailscaleTailnetNodeName,
    #[facet(rename = "TEAMY_BROKER_JAVA")]
    pub java: JavaExecutablePath,
}

impl PhoneProfileV1 {
    pub const SCHEMA_VERSION: u32 = 1;

    pub fn into_profile(self) -> eyre::Result<PhoneProfile> {
        eyre::ensure!(
            self.schema_version == Self::SCHEMA_VERSION,
            "expected phone profile schema_version 1, got {}",
            self.schema_version
        );
        let profile = PhoneProfile {
            desktop: self.desktop,
            phone: self.phone,
            desktop_node: self.desktop_node,
            phone_node: self.phone_node,
            user: self.user,
            phone_name: self.phone_name,
            java: self.java,
        };
        profile.validate()?;
        Ok(profile)
    }
}

impl From<&PhoneProfile> for PhoneProfileV1 {
    fn from(profile: &PhoneProfile) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            desktop: profile.desktop,
            phone: profile.phone,
            desktop_node: profile.desktop_node.clone(),
            phone_node: profile.phone_node.clone(),
            user: profile.user,
            phone_name: profile.phone_name.clone(),
            java: profile.java.clone(),
        }
    }
}
