#[derive(facet::Facet, Debug)]
pub(super) struct TailscaleStatus {
    #[facet(rename = "BackendState")]
    pub state: String,
    #[facet(rename = "Self")]
    pub node: super::tailscale_self::TailscaleSelf,
}
