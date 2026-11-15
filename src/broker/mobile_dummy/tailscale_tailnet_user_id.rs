use facet::Facet;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

/// Nonzero user identity expected in the local Tailscale status and peer response.
#[derive(Clone, Copy, PartialEq, Eq, Facet)]
#[facet(opaque, proxy = u64)]
pub struct TailscaleTailnetUserId(u64);

impl TailscaleTailnetUserId {
    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for TailscaleTailnetUserId {
    type Error = eyre::Report;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        eyre::ensure!(value != 0, "tailnet user ID rejected");
        Ok(Self(value))
    }
}

impl From<&TailscaleTailnetUserId> for u64 {
    fn from(value: &TailscaleTailnetUserId) -> Self {
        value.0
    }
}

impl Debug for TailscaleTailnetUserId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("TailscaleTailnetUserId([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_accepts_numeric_and_legacy_quoted_ids() {
        for json in ["42", r#""42""#] {
            let parsed: TailscaleTailnetUserId = facet_json::from_str(json).unwrap();
            assert_eq!(parsed.get(), 42);
            assert_eq!(facet_json::to_string(&parsed).unwrap(), "42");
            assert_eq!(format!("{parsed:?}"), "TailscaleTailnetUserId([redacted])");
        }
    }

    #[test]
    fn construction_and_json_reject_zero_and_invalid_numbers() {
        let error = TailscaleTailnetUserId::try_from(0).unwrap_err();
        assert_eq!(error.to_string(), "tailnet user ID rejected");
        assert_eq!(
            TailscaleTailnetUserId::try_from(u64::MAX).unwrap().get(),
            u64::MAX
        );
        for json in ["0", r#""0""#, "-1", "18446744073709551616", r#""invalid""#] {
            let _ = facet_json::from_str::<TailscaleTailnetUserId>(json).unwrap_err();
        }
    }
}
