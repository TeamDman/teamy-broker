use std::fmt;
use uuid::Uuid;

/// A validated lease identity. Keep its spelling for compatibility with stored descriptors.
#[derive(Clone, PartialEq, Eq)]
pub struct LeaseId(String);

impl TryFrom<String> for LeaseId {
    type Error = eyre::Report;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Uuid::parse_str(&value).map_err(|_redacted_error| eyre::eyre!("invalid lease ID"))?;
        Ok(Self(value))
    }
}

impl LeaseId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Debug for LeaseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LeaseId([redacted])")
    }
}
