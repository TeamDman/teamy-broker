use crate::broker::app_request::AppRequest;
use arbitrary::Arbitrary;
use facet::Facet;
use figue::{self as args};
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct CleanupArgs {
    #[facet(args::named)]
    pub lease_id: String,
}

impl CleanupArgs {
    /// # Errors
    /// Rejects malformed lease UUIDs before any cleanup effect.
    pub fn into_request(self) -> eyre::Result<AppRequest> {
        Ok(AppRequest::Cleanup(self.lease_id.try_into()?))
    }
}
