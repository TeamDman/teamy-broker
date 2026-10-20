use crate::protocol::AcquireRequest;
#[derive(Debug)]
pub struct CredentialPermission {
    request: AcquireRequest,
    supply: bool,
    until: std::time::Instant,
    pub(crate) generation: Option<String>,
    pub(crate) synthetic_fixture: bool,
}
impl CredentialPermission {
    /// # Errors
    /// Rejects invalid metadata, unavailable state, cancellation or expiry.
    pub fn new(request: AcquireRequest, supply: bool) -> eyre::Result<Self> {
        request.validate()?;
        eyre::ensure!(
            request.backend == crate::protocol::Backend::Memory,
            "phone permission requires memory-only lease"
        );
        Ok(Self {
            request,
            supply,
            generation: None,
            synthetic_fixture: false,
            until: std::time::Instant::now() + std::time::Duration::from_mins(5),
        })
    }
    #[must_use]
    pub fn matches(&self, request: &AcquireRequest) -> bool {
        self.live() && self.request == *request
    }
    #[must_use]
    pub fn live(&self) -> bool {
        self.until > std::time::Instant::now()
    }
    #[cfg(test)]
    pub(crate) fn expire_for_test(&mut self) {
        self.until = std::time::Instant::now();
    }
    #[must_use]
    pub const fn supply(&self) -> bool {
        self.supply
    }
}
