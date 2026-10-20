use facet::Facet;

#[derive(Facet, Clone, Copy, Debug, PartialEq, Eq)]
#[facet(rename_all = "snake_case")]
#[repr(u8)]
pub enum StopReason {
    StatusUnavailable,
    ActiveOperations,
    CachedLeases,
    PendingCleanup,
    AlreadyStopping,
}
