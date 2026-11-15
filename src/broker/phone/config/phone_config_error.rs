use std::error::Error;
use std::fmt;

/// Local configuration failures with safe, actionable process-boundary messages.
#[derive(Debug, Clone, Copy)]
pub enum PhoneConfigError {
    RelativeHome,
    MissingOrInvalidProfile,
    JavaUnavailable,
    BrokerActive,
    ProtectedIdentityUnavailable,
    PairedProfileChange,
    PublicationFailed,
}

impl PhoneConfigError {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::RelativeHome => "Application home must be an absolute path",
            Self::MissingOrInvalidProfile => {
                "Phone profile is missing or invalid; run phone-configure"
            }
            Self::JavaUnavailable => "Configured Java executable is unavailable",
            Self::BrokerActive => "Stop the broker before changing phone configuration",
            Self::ProtectedIdentityUnavailable => {
                "Protected phone identity is unavailable; configuration was not changed"
            }
            Self::PairedProfileChange => "Phone is paired; revoke it before changing configuration",
            Self::PublicationFailed => {
                "Phone configuration could not be saved; previous settings remain"
            }
        }
    }
}

impl fmt::Display for PhoneConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl Error for PhoneConfigError {}
