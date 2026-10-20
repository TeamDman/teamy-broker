use facet::Facet;
/// One explicit presentation attempt, independent of foreground permission.
#[derive(Clone, Debug, Facet, PartialEq, Eq)]
pub struct ApprovalPresentationOutcome {
    pub visible_before: bool,
    pub show_requested: bool,
    pub show_succeeded: Option<bool>,
    pub show_error_code: Option<u32>,
    pub visible_after: bool,
}
