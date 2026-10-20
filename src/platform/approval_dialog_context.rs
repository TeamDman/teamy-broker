use crate::protocol::ApprovalAttentionOutcome;
use crate::protocol::ApprovalPreviewEvent;
use crate::protocol::ApprovalPreviewEventKind;
use std::sync::Arc;
use std::sync::Mutex;
use windows_sys::Win32::Foundation::HWND;
#[derive(Debug)]
pub(crate) struct ApprovalDialogContext<'a> {
    pub state: &'a super::approval_dialog_state::ApprovalDialogState,
    pub shared: Option<Arc<crate::broker::shared_preview::SharedPreview>>,
    pub secret_input: Mutex<Option<usize>>,
    pub attention: super::approval_attention::DialogAttention,
    pub presentation: super::approval_presentation::DialogPresentation,
    pub presentation_outcome: Mutex<Option<crate::protocol::ApprovalPresentationOutcome>>,
    pub events: Option<Mutex<Vec<ApprovalPreviewEvent>>>,
    pub outcome: Mutex<Option<ApprovalAttentionOutcome>>,
}
impl ApprovalDialogContext<'_> {
    pub fn present_and_request_attention(&self, hwnd: HWND) {
        if let Some(outcome) = self.presentation.show_if_needed(hwnd) {
            tracing::info!(
                visible_before = outcome.visible_before,
                show_requested = outcome.show_requested,
                show_succeeded = outcome.show_succeeded,
                show_error_code = outcome.show_error_code,
                visible_after = outcome.visible_after,
                "Native approval presentation"
            );
            if self.events.is_some() {
                *self
                    .presentation_outcome
                    .lock()
                    .expect("preview presentation lock") = Some(outcome);
            }
        }
        if let Some(outcome) = self.attention.request_if_visible(hwnd) {
            tracing::info!(
                visible_before = outcome.visible_before,
                foreground_returned = outcome.foreground_returned,
                foreground_matches = outcome.foreground_matches,
                sound_queued = outcome.sound_queued,
                sound_error_code = outcome.sound_error_code,
                flash_requested = outcome.flash_previously_active.is_some(),
                flash_previously_active = outcome.flash_previously_active,
                "Native approval attention"
            );
            if self.events.is_some() {
                self.record(hwnd, ApprovalPreviewEventKind::VisibleAttention, None);
                *self.outcome.lock().expect("preview attention lock") = Some(outcome);
            }
        }
    }
    pub fn record(&self, hwnd: HWND, kind: ApprovalPreviewEventKind, button: Option<i32>) {
        if let Some(events) = &self.events {
            let mut events = events.lock().expect("preview event lock");
            if events.len() < 8 {
                events.push(super::approval_preview_capture::capture(hwnd, kind, button));
            }
        }
    }
}
