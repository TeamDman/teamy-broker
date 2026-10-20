#[derive(facet::Facet, Debug)]
pub(super) struct TailscaleNode {
    #[facet(rename = "StableID")]
    pub stable_id: String,
    #[facet(rename = "Name")]
    pub name: String,
    #[facet(rename = "User")]
    pub user: u64,
    #[facet(rename = "KeyExpiry")]
    pub key_expiry: String,
    #[facet(rename = "Addresses")]
    pub addresses: Vec<String>,
    #[facet(rename = "Hostinfo")]
    pub host: super::tailscale_host::TailscaleHost,
}
