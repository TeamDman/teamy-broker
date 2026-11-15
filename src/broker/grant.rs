use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::time::Instant;
use zeroize::Zeroizing;
pub struct Grant {
    /// Keeps the request visible until grant handoff or abandonment finishes.
    pub(crate) _work: super::work_guard::WorkGuard,
    pub id: String,
    pub expires: u64,
    pub deadline: Instant,
    pub bytes: Zeroizing<Vec<u8>>,
}
impl Debug for Grant {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("Grant([redacted])")
    }
}
