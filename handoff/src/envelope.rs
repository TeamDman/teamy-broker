use facet::Facet;
/// Only ciphertext is reflected. Plaintext secret types have no Facet or serialization.
#[derive(Facet)]
pub struct Envelope {
    pub nonce: String,
    pub ciphertext: String,
}
impl std::fmt::Debug for Envelope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Envelope([redacted])")
    }
}
