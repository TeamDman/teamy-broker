use zeroize::Zeroizing;
pub struct Grant {
    pub id: String,
    pub expires: u64,
    pub deadline: std::time::Instant,
    pub bytes: Zeroizing<Vec<u8>>,
}
impl std::fmt::Debug for Grant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Grant([redacted])")
    }
}
