use crate::broker::phone::pairing_confirmation::Attempt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Instant;
use zeroize::Zeroizing;
pub(super) struct PairingPromptState {
    pub code: Zeroizing<String>,
    pub confirmation: Option<Arc<Attempt>>,
    pub until: Instant,
    pub closed: Arc<AtomicBool>,
}
impl Debug for PairingPromptState {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("PairingPromptState([redacted])")
    }
}
impl PairingPromptState {
    pub fn on_timer(&self, present: impl FnOnce()) -> bool {
        if self.confirmation.as_ref().is_some_and(|a| a.finished()) {
            return true;
        }
        if self.closed.load(Ordering::Acquire) || Instant::now() >= self.until {
            return true;
        }
        present();
        false
    }
}
#[cfg(test)]
#[path = "pairing_prompt_state_tests.rs"]
mod tests;
