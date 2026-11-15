use std::fmt;
use uuid::Uuid;

/// An exact pairing-attempt identity, distinct from a credential lease.
#[derive(Clone, PartialEq, Eq)]
pub struct PairingAttemptId(String);

impl TryFrom<String> for PairingAttemptId {
    type Error = eyre::Report;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Uuid::parse_str(&value).map_err(|_redacted_error| eyre::eyre!("invalid attempt ID"))?;
        Ok(Self(value))
    }
}

impl PairingAttemptId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PairingAttemptId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PairingAttemptId([redacted])")
    }
}
