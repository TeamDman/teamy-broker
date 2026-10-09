use arbitrary::Arbitrary;
use facet::Facet;
use figue::{self as args};
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct CleanupArgs {
    #[facet(args::named)]
    pub lease_id: String,
}
