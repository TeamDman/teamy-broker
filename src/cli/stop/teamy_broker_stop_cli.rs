use crate::broker::app_request::AppRequest;
use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct StopArgs {
    /// Explicitly interrupt active work and discard cached leases for maintenance.
    #[facet(args::named, default)]
    pub force: bool,
}

impl StopArgs {
    #[must_use]
    pub const fn into_request(self) -> AppRequest {
        if self.force {
            AppRequest::StopForce
        } else {
            AppRequest::StopWhenIdle
        }
    }
}
