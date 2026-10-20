use facet::Facet;
#[derive(Clone, Copy, Debug, Facet, PartialEq, Eq)]
#[repr(u8)]
pub enum ApprovalPreviewDisposition {
    ApprovedTest,
    DeniedTest,
    Closed,
    Cancelled,
    TimedOut,
    Unavailable,
}
