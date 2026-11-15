use crate::broker::app_request::AppRequest;
use arbitrary::Arbitrary;
use facet::Facet;
use figue::{self as args};
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct PreviewArgs {
    /// Open one fixed TEST/DUMMY resident dialog for at most 60 seconds; no vault or lease.
    #[facet(args::named)]
    pub show_test_dialog: bool,
}

impl PreviewArgs {
    #[must_use]
    pub const fn into_request(self) -> AppRequest {
        if self.show_test_dialog {
            AppRequest::ApprovalPreview
        } else {
            AppRequest::PreviewProposal
        }
    }
}
