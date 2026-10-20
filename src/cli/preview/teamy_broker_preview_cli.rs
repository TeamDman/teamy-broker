use arbitrary::Arbitrary;
use facet::Facet;
use figue::{self as args};
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct PreviewArgs {
    /// Open one fixed TEST/DUMMY resident dialog for at most 60 seconds; no vault or lease.
    #[facet(args::named)]
    pub show_test_dialog: bool,
}
