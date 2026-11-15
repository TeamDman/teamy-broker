use crate::protocol::Backend;
use facet::Facet;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
#[derive(Facet)]
pub(crate) struct Descriptor {
    pub id: String,
    pub backend: Backend,
    pub expires: u64,
}
impl Debug for Descriptor {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("Descriptor([redacted])")
    }
}
