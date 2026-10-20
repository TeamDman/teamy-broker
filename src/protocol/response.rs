use facet::Facet;
#[derive(Facet, Debug, Default)]
pub struct Response {
    pub code: String,
    pub failure: Option<super::FailureKind>,
    pub lease_id: Option<String>,
    pub expires_unix_ms: Option<u64>,
    pub remaining_millis: Option<u64>,
    pub status: Option<super::Status>,
}
impl Response {
    #[must_use]
    pub fn failure(kind: super::FailureKind) -> Self {
        Self {
            code: "failed".into(),
            failure: Some(kind),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn code(code: &str) -> Self {
        Self {
            code: code.into(),
            ..Self::default()
        }
    }
}
