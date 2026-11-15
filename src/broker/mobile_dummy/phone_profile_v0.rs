use super::JavaExecutablePath;
use super::TailscaleTailnetIpv4Addr;
use super::TailscaleTailnetNodeId;
use super::TailscaleTailnetNodeName;
use super::TailscaleTailnetUserId;
use super::phone_profile_v1::PhoneProfileV1;
use facet::Facet;

#[derive(Debug, Facet)]
pub(super) struct PhoneProfileV0 {
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

impl PhoneProfileV0 {
    pub fn upgrade(self) -> PhoneProfileV1 {
        PhoneProfileV1 {
            schema_version: PhoneProfileV1::SCHEMA_VERSION,
            desktop: self.desktop,
            phone: self.phone,
            desktop_node: self.desktop_node,
            phone_node: self.phone_node,
            user: self.user,
            phone_name: self.phone_name,
            java: self.java,
        }
    }
}
