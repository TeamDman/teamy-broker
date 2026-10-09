use super::lease::Lease;
pub trait Storage: Send + Sync {
    /// # Errors
    ///
    /// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
    fn publish(&self, lease: &Lease) -> eyre::Result<()>;
    /// # Errors
    ///
    /// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
    fn remove(&self, lease: &Lease) -> eyre::Result<()>;
}
