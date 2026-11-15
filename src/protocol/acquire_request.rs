use super::Backend;
use facet::Facet;
use std::fmt;
#[derive(Facet, Clone, PartialEq, Eq)]
pub struct AcquireRequest {
    pub account: String,
    pub secret_ref: String,
    pub purpose: String,
    pub requesting_app: String,
    pub destination: String,
    pub backend: Backend,
}
impl fmt::Debug for AcquireRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AcquireRequest([redacted])")
    }
}
impl AcquireRequest {
    /// # Errors
    ///
    /// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
    pub fn validate(&self) -> eyre::Result<()> {
        self.backend.validate()?;
        for s in [
            &self.account,
            &self.secret_ref,
            &self.purpose,
            &self.requesting_app,
            &self.destination,
        ] {
            eyre::ensure!(
                !s.is_empty() && s.len() <= 1024 && !s.chars().any(char::is_control),
                "invalid request metadata"
            );
        }
        eyre::ensure!(
            !self.account.starts_with('-')
                && self.secret_ref.starts_with("op://")
                && self.secret_ref[5..].split('/').count() >= 3,
            "explicit account and item field reference required"
        );
        Ok(())
    }
}
