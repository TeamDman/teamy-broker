use facet::Facet;
use zeroize::Zeroize;
#[derive(Facet)]
pub struct PairingInspection {
    pub attempt_id: String,
    pub expires_unix_ms: u64,
    pub comparison: String,
    pub remaining_wrong_attempts: u8,
}
impl std::fmt::Debug for PairingInspection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PairingInspection([redacted])")
    }
}
impl Drop for PairingInspection {
    fn drop(&mut self) {
        self.comparison.zeroize();
    }
}
#[derive(Facet)]
pub struct PairingCode {
    pub comparison: String,
}
impl std::fmt::Debug for PairingCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PairingCode([redacted])")
    }
}
impl Drop for PairingCode {
    fn drop(&mut self) {
        self.comparison.zeroize();
    }
}
#[derive(Facet, Debug)]
pub struct PairingConfirmationResult {
    pub attempt_id: String,
    pub accepted: bool,
}
