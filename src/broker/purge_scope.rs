use super::lease_id::LeaseId;

#[derive(Debug, PartialEq, Eq)]
pub enum PurgeScope {
    All,
    Lease(LeaseId),
}
