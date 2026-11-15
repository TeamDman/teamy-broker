#[derive(facet::Facet, Debug)]
pub(super) struct TailscaleHost {
    #[facet(rename = "OS")]
    pub os: String,
}
