use crate::broker::app_request::AppRequest;
use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct PhonePairInspectArgs {
    /// Exact active attempt UUID returned by phone-pair. No latest or wildcard.
    #[facet(args::named)]
    pub attempt_id: String,
}

impl PhonePairInspectArgs {
    /// # Errors
    /// Rejects malformed pairing-attempt UUIDs without contacting the broker.
    pub fn into_request(self) -> eyre::Result<AppRequest> {
        Ok(AppRequest::PairingInspect(self.attempt_id.try_into()?))
    }
}
