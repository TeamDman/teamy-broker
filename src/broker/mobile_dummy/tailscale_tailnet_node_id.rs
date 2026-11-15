use facet::Facet;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

/// Configured Tailscale node identity. Live peer verification remains separate.
/// The private string and validating proxy preserve the profile's input rules.
#[derive(Clone, PartialEq, Eq, Facet)]
#[facet(opaque, proxy = String)]
pub struct TailscaleTailnetNodeId(String);

impl TailscaleTailnetNodeId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TailscaleTailnetNodeId {
    type Error = eyre::Report;

    /// # Errors
    /// Rejects empty, oversized, or unsupported node IDs without revealing them.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        eyre::ensure!(
            !value.is_empty()
                && value.len() <= 255
                && value
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c)),
            "tailnet node ID rejected"
        );
        Ok(Self(value))
    }
}

impl Debug for TailscaleTailnetNodeId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("TailscaleTailnetNodeId([redacted])")
    }
}

impl From<&TailscaleTailnetNodeId> for String {
    fn from(value: &TailscaleTailnetNodeId) -> Self {
        value.0.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::TailscaleTailnetNodeId;

    #[test]
    fn accepts_existing_profile_syntax_and_length_boundary() {
        for value in ["test-Node_42.example".to_owned(), "a".repeat(255)] {
            let node = TailscaleTailnetNodeId::try_from(value.clone()).unwrap();
            assert_eq!(node.as_str(), value);
        }
    }

    #[test]
    fn rejects_invalid_ids_without_disclosing_input() {
        for value in [
            String::new(),
            "a".repeat(256),
            "private node".to_owned(),
            "private/node".to_owned(),
            "private\nnode".to_owned(),
            "privaté".to_owned(),
        ] {
            let error = TailscaleTailnetNodeId::try_from(value).unwrap_err();
            assert_eq!(error.to_string(), "tailnet node ID rejected");
        }
    }

    #[test]
    fn debug_redacts_identity() {
        let node = TailscaleTailnetNodeId::try_from("private-test-node".to_owned()).unwrap();
        assert_eq!(format!("{node:?}"), "TailscaleTailnetNodeId([redacted])");
    }

    #[test]
    fn json_uses_a_string_and_enforces_constructor_validation() {
        let node: TailscaleTailnetNodeId = facet_json::from_str(r#""test-node_42""#).unwrap();
        assert_eq!(node.as_str(), "test-node_42");
        assert_eq!(facet_json::to_string(&node).unwrap(), r#""test-node_42""#);
        for json in [
            r#""""#,
            r#""private node""#,
            r#""private/node""#,
            r#"{"0":"test-node"}"#,
        ] {
            let _ = facet_json::from_str::<TailscaleTailnetNodeId>(json).unwrap_err();
        }
    }
}
