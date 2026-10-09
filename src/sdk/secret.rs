use zeroize::Zeroizing;
/// Cooperative lease handle. Intentional caller copies are outside broker revocation.
pub struct Secret {
    bytes: Zeroizing<Vec<u8>>,
    expires: u64,
    deadline: std::time::Instant,
    pub lease_id: String,
}
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret([redacted])")
    }
}
impl Secret {
    pub(crate) fn new(
        bytes: Zeroizing<Vec<u8>>,
        expires: u64,
        lease_id: String,
        started: std::time::Instant,
        broker_remaining: u64,
    ) -> eyre::Result<Self> {
        eyre::ensure!(
            bytes.len() <= crate::protocol::MAX_SECRET,
            "oversized secret"
        );
        let now = crate::protocol::wire::unix_ms()?;
        let wall_remaining = expires
            .checked_sub(now)
            .filter(|r| *r > 0 && *r <= crate::protocol::LEASE_MILLIS)
            .ok_or_else(|| eyre::eyre!("invalid or expired lease"))?;
        eyre::ensure!(
            broker_remaining > 0 && broker_remaining <= crate::protocol::LEASE_MILLIS,
            "invalid broker lease duration"
        );
        // Subtract the entire local request duration: a conservative bound on transfer delay.
        let elapsed = u64::try_from(started.elapsed().as_millis())?;
        let remaining = wall_remaining.min(
            broker_remaining
                .checked_sub(elapsed)
                .filter(|n| *n > 0)
                .ok_or_else(|| eyre::eyre!("lease expired in transit"))?,
        );
        Ok(Self {
            bytes,
            expires,
            deadline: (std::time::Instant::now() + std::time::Duration::from_millis(remaining))
                .min(started + std::time::Duration::from_millis(crate::protocol::LEASE_MILLIS)),
            lease_id,
        })
    }
    /// Bytes are deliberately exposed only to a caller-supplied closure after both hard-deadline checks.
    /// # Errors
    ///
    /// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
    pub fn expose<T>(&self, use_bytes: impl FnOnce(&[u8]) -> T) -> eyre::Result<T> {
        eyre::ensure!(
            crate::protocol::wire::unix_ms()? < self.expires
                && std::time::Instant::now() < self.deadline,
            "lease expired"
        );
        Ok(use_bytes(&self.bytes))
    }
    #[must_use]
    pub fn expires_unix_ms(&self) -> u64 {
        self.expires
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sdk_rejects_expired_and_oversized_leases_and_monotonic_expiry() {
        let now = crate::protocol::wire::unix_ms().unwrap();
        let _ = Secret::new(
            Zeroizing::new(b"SYNTHETIC_SECRET_NEVER_LOG".to_vec()),
            now - 1,
            "synthetic".into(),
            std::time::Instant::now(),
            1000,
        )
        .unwrap_err();
        let _ = Secret::new(
            Zeroizing::new(vec![1]),
            now + crate::protocol::LEASE_MILLIS + 5000,
            "synthetic".into(),
            std::time::Instant::now(),
            1000,
        )
        .unwrap_err();
        let mut secret = Secret::new(
            Zeroizing::new(vec![1]),
            now + 1000,
            "synthetic".into(),
            std::time::Instant::now(),
            1000,
        )
        .unwrap();
        secret.deadline = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(1))
            .unwrap();
        let _ = secret.expose(<[u8]>::len).unwrap_err();
        assert!(!format!("{secret:?}").contains("SYNTHETIC"));
    }
    #[test]
    fn broker_remaining_duration_bounds_a_cached_handle_even_when_wall_time_is_far_from_expiry() {
        let now = crate::protocol::wire::unix_ms().unwrap();
        let secret = Secret::new(
            Zeroizing::new(vec![1]),
            now + 300_000,
            "synthetic".into(),
            std::time::Instant::now(),
            1000,
        )
        .unwrap();
        assert!(
            secret
                .deadline
                .saturating_duration_since(std::time::Instant::now())
                <= std::time::Duration::from_secs(1)
        );
    }
}
