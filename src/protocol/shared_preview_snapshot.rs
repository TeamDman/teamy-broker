use facet::Facet;
/// Fixed no-vault request state. Opaque identity changes on every request and process restart.
#[derive(Clone, Debug, Facet, PartialEq, Eq)]
pub struct SharedPreviewSnapshot {
    pub kind: String,
    pub request_id: String,
    pub expires_unix_ms: u64,
    #[facet(default, skip_serializing_if = Option::is_none)]
    pub credential: Option<Box<super::credential_context::CredentialContext>>,
}
#[derive(Clone, Debug, Facet)]
pub struct SharedPreviewState {
    pub pending: Option<SharedPreviewSnapshot>,
}
