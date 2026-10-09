use facet::Facet;
#[derive(Facet, Debug, Default)]
pub struct Status {
    pub running: bool,
    pub active_leases: u64,
    pub cleanup_failures: u64,
}
