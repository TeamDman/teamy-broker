use facet::Facet;
/// A preview has no secret, lease or storage destination, even when approved.
#[derive(Clone, Debug, Facet)]
pub struct ApprovalPreviewResult {
    pub disposition: super::ApprovalPreviewDisposition,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub broker_pid: u32,
    pub connected_client_pid: u32,
    pub broker_executable: String,
    pub broker_sha256: Option<String>,
    pub task_dialog_hresult: i32,
    pub selected_button: i32,
    pub startup_flags: u32,
    pub startup_show_window: u16,
    pub presentation: Option<super::ApprovalPresentationOutcome>,
    pub attention: Option<super::ApprovalAttentionOutcome>,
    pub events: Vec<super::ApprovalPreviewEvent>,
}
