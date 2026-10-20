#[derive(facet::Facet, Debug)]
pub(super) struct TailscaleSelf {
    #[facet(rename = "ID")]
    pub id: String,
    #[facet(rename = "OS")]
    pub os: String,
    #[facet(rename = "UserID")]
    pub user_id: u64,
    #[facet(rename = "TailscaleIPs")]
    pub ips: Vec<String>,
}
