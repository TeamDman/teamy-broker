use facet::Facet;
#[derive(Clone, Copy, Debug, Facet, PartialEq, Eq)]
#[repr(u8)]
pub enum ApprovalPreviewEventKind {
    Created,
    VisibleAttention,
    DeadlineOrCancellation,
    ButtonClicked,
    Destroyed,
}
