use crate::protocol::ApprovalAttentionOutcome;
use crate::protocol::ApprovalPresentationOutcome;
use crate::protocol::ApprovalPreviewEvent;
use std::sync::Mutex;
/// Only populated by the synthetic private-desktop probe; never receives live comparison text.
#[derive(Debug, Default)]
pub(super) struct PairingPromptDiagnostics {
    pub events: Mutex<Vec<ApprovalPreviewEvent>>,
    pub presentation: Mutex<Option<ApprovalPresentationOutcome>>,
    pub attention: Mutex<Option<ApprovalAttentionOutcome>>,
    pub protection: Mutex<Option<bool>>,
    pub hresult: Mutex<i32>,
    pub button: Mutex<i32>,
}
