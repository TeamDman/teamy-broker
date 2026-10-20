use facet::Facet;
/// Independent presentation observations; contains no request or credential data.
#[derive(Clone, Debug, Facet, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent immutable visibility, foreground and sound observations"
)]
pub struct ApprovalAttentionOutcome {
    pub visible_before: bool,
    pub foreground_returned: bool,
    pub foreground_matches: bool,
    pub sound_queued: bool,
    pub sound_error_code: Option<u32>,
    /// The prior active state returned by `FlashWindowEx`, not success.
    pub flash_previously_active: Option<bool>,
}
