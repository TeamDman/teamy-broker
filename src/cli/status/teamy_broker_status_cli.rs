use arbitrary::Arbitrary;
use facet::Facet;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct StatusArgs;
