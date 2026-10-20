use facet::Facet;
/// Metadata from only the fixed TEST/DUMMY dialog; no credential fields.
#[derive(Clone, Debug, Facet)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent immutable window observations, not mutable state flags"
)]
pub struct ApprovalPreviewEvent {
    pub kind: super::ApprovalPreviewEventKind,
    pub unix_ms: u64,
    pub hwnd: u64,
    pub pid: u32,
    pub thread_id: u32,
    pub session_id: Option<u32>,
    pub desktop: String,
    pub window_station: String,
    pub input_desktop: String,
    pub title: String,
    pub class: String,
    pub rect: Option<[i32; 4]>,
    pub monitor_rect: Option<[i32; 4]>,
    pub workarea_rect: Option<[i32; 4]>,
    pub monitor_intersects: bool,
    pub workarea_intersects: bool,
    pub visible: bool,
    pub minimized: bool,
    pub cloaked: Option<u32>,
    pub foreground_matches: bool,
    pub button: Option<i32>,
}
