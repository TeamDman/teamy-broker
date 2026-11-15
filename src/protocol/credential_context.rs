use facet::Facet;
use std::fmt;
#[derive(Clone, Facet, PartialEq, Eq)]
pub struct CredentialContext {
    pub request: super::AcquireRequest,
    pub supply: bool,
    #[facet(default)]
    pub synthetic_fixture: bool,
    pub connected_process_id: u32,
    pub provider_state: String,
}
impl fmt::Debug for CredentialContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CredentialContext([metadata redacted])")
    }
}
