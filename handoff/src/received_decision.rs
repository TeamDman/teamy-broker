use crate::clock::Clock;
use crate::deadline::Deadline;
use std::sync::Arc;
use zeroize::Zeroizing;
/// Queue delivery is provisional. The approval receiver must recheck expiry when resumed.
pub struct ReceivedDecision {
    pub(crate) bytes: Option<Zeroizing<Vec<u8>>>,
    pub(crate) deadline: Deadline,
    pub(crate) clock: Arc<dyn Clock>,
}
impl std::fmt::Debug for ReceivedDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReceivedDecision([redacted])")
    }
}
impl ReceivedDecision {
    /// Consumes the provisional decision and clears its dummy bytes even on expiry.
    pub fn approved(self) -> eyre::Result<bool> {
        eyre::ensure!(
            self.deadline.valid_at(self.clock.reading()?),
            "request expired"
        );
        Ok(self.bytes.is_some())
    }
}
