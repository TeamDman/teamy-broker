use arbitrary::Arbitrary;
use facet::Facet;
use figue::{self as args};
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct PurgeArgs {
    /// Purge all leases. Mutually exclusive with --lease-id.
    #[facet(args::named, default)]
    pub all: bool,
    /// Opaque lease ID returned by SDK. Mutually exclusive with --all.
    #[facet(args::named)]
    pub lease_id: Option<String>,
}
