use facet::Facet;
/// Closed, safe failure taxonomy. Contains no item, destination, credential or upstream detail.
#[derive(Facet, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FailureKind {
    UserDenied,
    ShutdownBlocked,
    ApprovalTimedOut,
    ApprovalUnavailable,
    UpstreamReadFailed,
    RequestCancelled,
    LeaseExpired,
    CachePublishFailed,
    RequestTimedOut,
    RequestFailed,
}
impl FailureKind {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::ShutdownBlocked => {
                "Broker has active work, cached leases, or pending cleanup; inspect status or explicitly use stop --force"
            }
            Self::UserDenied => "Credential approval denied by user",
            Self::ApprovalTimedOut => "Credential approval timed out",
            Self::ApprovalUnavailable => "Credential approval UI unavailable",
            Self::UpstreamReadFailed => "Upstream credential read failed",
            Self::RequestCancelled => "Credential request cancelled",
            Self::LeaseExpired => "Credential lease expired",
            Self::CachePublishFailed => "Credential cache publication failed",
            Self::RequestTimedOut => "Credential request timed out",
            Self::RequestFailed => "Credential request failed",
        }
    }
}
impl std::fmt::Display for FailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}
impl std::error::Error for FailureKind {}
