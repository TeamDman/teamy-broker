use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct StopArgs {
    /// Explicitly interrupt active work and discard cached leases for maintenance.
    #[facet(args::named, default)]
    pub force: bool,
}
