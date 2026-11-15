use super::phone_authorization_mode::PhoneAuthorizationMode;
use super::synthetic_credential_fixture;
use crate::protocol::AcquireRequest;

/// Validated metadata and permission; no wire operation names or overloaded ID fields.
#[derive(Debug)]
pub struct PhoneAuthorizationRequest {
    acquire: AcquireRequest,
    mode: PhoneAuthorizationMode,
}

impl PhoneAuthorizationRequest {
    /// # Errors
    /// Rejects invalid acquisition metadata or a changed synthetic fixture.
    pub fn new(acquire: AcquireRequest, mode: PhoneAuthorizationMode) -> eyre::Result<Self> {
        acquire.validate()?;
        if mode == PhoneAuthorizationMode::SyntheticFixture {
            synthetic_credential_fixture::validate(&acquire)?;
        }
        Ok(Self { acquire, mode })
    }

    pub(crate) fn into_parts(self) -> (AcquireRequest, PhoneAuthorizationMode) {
        (self.acquire, self.mode)
    }
}
