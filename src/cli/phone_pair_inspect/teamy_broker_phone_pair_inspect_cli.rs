use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct PhonePairInspectArgs {
    /// Exact active attempt UUID returned by phone-pair. No latest or wildcard.
    #[facet(args::named)]
    pub attempt_id: String,
}
