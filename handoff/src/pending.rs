use facet::Facet;
#[derive(Facet)]
pub struct Pending {
    /// Exact UTF-8 AAD for the response, including desktop, peer, scope and expiry.
    pub binding: String,
}
impl std::fmt::Debug for Pending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Pending([redacted])")
    }
}
