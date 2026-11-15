use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::sync::Arc;
use zeroize::Zeroizing;
pub enum ApprovalDecision {
    Denied,
    DesktopRead,
    Supplied(Zeroizing<Vec<u8>>),
    Shared(Arc<super::shared_preview::SharedPreview>),
}
impl Debug for ApprovalDecision {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("ApprovalDecision([redacted])")
    }
}
