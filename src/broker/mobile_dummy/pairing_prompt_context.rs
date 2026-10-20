use super::pairing_prompt_diagnostics::PairingPromptDiagnostics;
use super::pairing_prompt_state::PairingPromptState;
use crate::platform::approval_attention::DialogAttention;
use crate::platform::approval_presentation::DialogPresentation;
use crate::protocol::ApprovalPreviewEventKind;
use windows_sys::Win32::Foundation::HWND;
#[derive(Debug)]
pub(super) struct PairingPromptContext<'a> {
    pub state: &'a PairingPromptState,
    pub presentation: DialogPresentation,
    pub attention: DialogAttention,
    pub diagnostics: Option<&'a PairingPromptDiagnostics>,
    pub presentation_enabled: bool,
    pub trace: Option<std::sync::Arc<super::preview_phase::PreviewTrace>>,
    pub visible_reported: std::sync::atomic::AtomicBool,
}
impl PairingPromptContext<'_> {
    pub fn record(&self, hwnd: HWND, kind: ApprovalPreviewEventKind, button: Option<i32>) {
        if let Some(diagnostics) = self.diagnostics {
            let mut events = diagnostics
                .events
                .lock()
                .expect("synthetic pairing event lock");
            if events.len() < 8 {
                events.push(crate::platform::approval_preview_capture::capture(
                    hwnd, kind, button,
                ));
            }
        }
    }
    fn visible(&self, trace: &super::preview_phase::PreviewTrace) {
        if !self
            .visible_reported
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            trace.emit(
                super::preview_phase::PreviewPhase::DialogVisible,
                super::preview_phase::PreviewMetadata {
                    visible: Some(true),
                    ..Default::default()
                },
            );
        }
    }
    pub fn present(&self, hwnd: HWND) {
        if !self.presentation_enabled {
            return;
        }
        if let Some(outcome) = self.presentation.show_if_needed(hwnd) {
            if let Some(trace) = &self.trace {
                if outcome.visible_after {
                    self.visible(trace);
                } else {
                    trace.emit(
                        super::preview_phase::PreviewPhase::DialogNotVisible,
                        super::preview_phase::PreviewMetadata {
                            visible: Some(false),
                            ..Default::default()
                        },
                    );
                }
            }
            tracing::info!(
                visible_before = outcome.visible_before,
                show_requested = outcome.show_requested,
                show_succeeded = outcome.show_succeeded,
                show_error_code = outcome.show_error_code,
                visible_after = outcome.visible_after,
                "Native pairing dialog presentation"
            );
            if let Some(diagnostics) = self.diagnostics {
                *diagnostics
                    .presentation
                    .lock()
                    .expect("synthetic pairing presentation lock") = Some(outcome);
            }
        }
        if let Some(outcome) = self.attention.request_if_visible(hwnd) {
            if let Some(trace) = &self.trace {
                self.visible(trace);
                trace.emit(
                    super::preview_phase::PreviewPhase::DialogAttention,
                    super::preview_phase::PreviewMetadata {
                        visible: Some(outcome.visible_before),
                        foreground: Some(outcome.foreground_matches),
                        sound_queued: Some(outcome.sound_queued),
                        ..Default::default()
                    },
                );
            }
            tracing::info!(
                foreground_returned = outcome.foreground_returned,
                foreground_matches = outcome.foreground_matches,
                sound_queued = outcome.sound_queued,
                "Native pairing dialog attention"
            );
            self.record(hwnd, ApprovalPreviewEventKind::VisibleAttention, None);
            if let Some(diagnostics) = self.diagnostics {
                *diagnostics
                    .attention
                    .lock()
                    .expect("synthetic pairing attention lock") = Some(outcome);
            }
        }
    }
}
