use zeroize::Zeroizing;
pub enum ApprovalDecision {
    Denied,
    DesktopRead,
    Supplied(Zeroizing<Vec<u8>>),
    Shared(std::sync::Arc<super::shared_preview::SharedPreview>),
}
impl std::fmt::Debug for ApprovalDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApprovalDecision([redacted])")
    }
}
