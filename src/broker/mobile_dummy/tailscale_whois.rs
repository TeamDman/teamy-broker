#[derive(facet::Facet, Debug)]
pub(super) struct TailscaleWhois {
    #[facet(rename = "Node")]
    pub node: super::tailscale_node::TailscaleNode,
}
