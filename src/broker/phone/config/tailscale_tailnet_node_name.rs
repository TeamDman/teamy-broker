use facet::Facet;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

/// Exact node name expected by the profile, preserving its existing input rules.
#[derive(Clone, PartialEq, Eq, Facet)]
#[facet(opaque, proxy = String)]
pub struct TailscaleTailnetNodeName(String);

impl TailscaleTailnetNodeName {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TailscaleTailnetNodeName {
    type Error = eyre::Report;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        eyre::ensure!(
            !value.is_empty()
                && value.len() <= 255
                && value
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c)),
            "tailnet node name rejected"
        );
        Ok(Self(value))
    }
}

impl From<&TailscaleTailnetNodeName> for String {
    fn from(value: &TailscaleTailnetNodeName) -> Self {
        value.0.clone()
    }
}

impl Debug for TailscaleTailnetNodeName {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("TailscaleTailnetNodeName([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_preserves_exact_node_names_and_length_boundary() {
        for name in ["Phone-test_42.invalid.".to_owned(), "a".repeat(255)] {
            let json = facet_json::to_string(&name).unwrap();
            let parsed: TailscaleTailnetNodeName = facet_json::from_str(&json).unwrap();
            assert_eq!(parsed.as_str(), name);
            assert_eq!(facet_json::to_string(&parsed).unwrap(), json);
            assert_eq!(
                format!("{parsed:?}"),
                "TailscaleTailnetNodeName([redacted])"
            );
        }
    }

    #[test]
    fn construction_and_json_reject_invalid_profile_names() {
        for name in [
            String::new(),
            "a".repeat(256),
            "private name".to_owned(),
            "private/name".to_owned(),
            "privaté".to_owned(),
        ] {
            let error = TailscaleTailnetNodeName::try_from(name.clone()).unwrap_err();
            assert_eq!(error.to_string(), "tailnet node name rejected");
            let _ = facet_json::from_str::<TailscaleTailnetNodeName>(
                &facet_json::to_string(&name).unwrap(),
            )
            .unwrap_err();
        }
    }
}
