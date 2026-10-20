use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use zeroize::Zeroizing;
pub(super) struct PairingPromptState {
    pub code: Zeroizing<String>,
    pub confirmation: Option<Arc<crate::broker::phone::pairing_confirmation::Attempt>>,
    pub until: std::time::Instant,
    pub closed: Arc<AtomicBool>,
}
impl std::fmt::Debug for PairingPromptState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PairingPromptState([redacted])")
    }
}
impl PairingPromptState {
    pub fn on_timer(&self, present: impl FnOnce()) -> bool {
        if self.confirmation.as_ref().is_some_and(|a| a.finished()) {
            return true;
        }
        if self.closed.load(std::sync::atomic::Ordering::Acquire)
            || std::time::Instant::now() >= self.until
        {
            return true;
        }
        present();
        false
    }
}
#[cfg(test)]
#[path = "pairing_prompt_state_tests.rs"]
mod tests;
