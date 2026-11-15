use crate::protocol::AcquireRequest;
use crate::protocol::LEASE_MILLIS;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::time::Duration;
use std::time::Instant;
use zeroize::Zeroizing;
/// Lease data intentionally has no reflection, serialization, or secret-bearing Debug.
pub struct Lease {
    pub id: String,
    pub request: AcquireRequest,
    pub expires: u64,
    pub deadline: Instant,
    pub secret: Zeroizing<Vec<u8>>,
}
impl Debug for Lease {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("Lease([redacted])")
    }
}
impl Lease {
    /// # Errors
    ///
    /// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
    pub fn new(
        request: AcquireRequest,
        secret: Zeroizing<Vec<u8>>,
        approved_at: u64,
        approved_tick: Instant,
    ) -> eyre::Result<Self> {
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            request,
            secret,
            expires: approved_at
                .checked_add(LEASE_MILLIS)
                .ok_or_else(|| eyre::eyre!("invalid clock"))?,
            deadline: approved_tick + Duration::from_millis(LEASE_MILLIS),
        })
    }
    #[must_use]
    pub fn valid(&self, now: u64) -> bool {
        now < self.expires && Instant::now() < self.deadline
    }
}
