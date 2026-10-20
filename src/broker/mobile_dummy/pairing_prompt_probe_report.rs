use facet::Facet;
/// Private-desktop synthetic geometry only; no comparison code, nonce, key or peer payload.
#[derive(Debug, Facet)]
pub struct PairingPromptProbeReport {
    pub process_pid: u32,
    pub startup_flags: u32,
    pub startup_show_window: u16,
    pub task_dialog_hresult: i32,
    pub selected_button: i32,
    pub accepted: bool,
    pub presentation_enabled: bool,
    pub protection_set: Option<bool>,
    pub presentation: Option<crate::protocol::ApprovalPresentationOutcome>,
    pub attention: Option<crate::protocol::ApprovalAttentionOutcome>,
    pub events: Vec<crate::protocol::ApprovalPreviewEvent>,
}
