use facet::Facet;
#[derive(Facet)]
pub(crate) struct Descriptor {
    pub id: String,
    pub backend: crate::protocol::Backend,
    pub expires: u64,
}
impl std::fmt::Debug for Descriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Descriptor([redacted])")
    }
}
