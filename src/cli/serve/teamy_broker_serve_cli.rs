use arbitrary::Arbitrary;
use facet::Facet;
use figue::{self as args};
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct ServeArgs {
    /// Explicit path to 1Password CLI. Default resolves op.exe through PATH.
    #[facet(args::named)]
    pub op_executable: Option<String>,
}
