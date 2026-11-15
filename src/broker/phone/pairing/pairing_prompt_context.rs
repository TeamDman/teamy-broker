use super::pairing_prompt_state::PairingPromptState;
use crate::platform::approval_attention::DialogAttention;
use crate::platform::approval_presentation::DialogPresentation;
use windows_sys::Win32::Foundation::HWND;

#[derive(Debug)]
pub(super) struct PairingPromptContext<'a> {
    pub state: &'a PairingPromptState,
    pub presentation: DialogPresentation,
    pub attention: DialogAttention,
}
impl PairingPromptContext<'_> {
    pub fn present(&self, hwnd: HWND) {
        if let Some(outcome) = self.presentation.show_if_needed(hwnd) {
            tracing::info!(
                visible_before = outcome.visible_before,
                show_requested = outcome.show_requested,
                show_succeeded = outcome.show_succeeded,
                show_error_code = outcome.show_error_code,
                visible_after = outcome.visible_after,
                "Native pairing dialog presentation"
            );
        }
        if let Some(outcome) = self.attention.request_if_visible(hwnd) {
            tracing::info!(
                foreground_returned = outcome.foreground_returned,
                foreground_matches = outcome.foreground_matches,
                sound_queued = outcome.sound_queued,
                "Native pairing dialog attention"
            );
        }
    }
}
