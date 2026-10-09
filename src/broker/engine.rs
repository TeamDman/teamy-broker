use super::adapter::Adapter;
use super::approval::Approval;
use super::grant::Grant;
use super::lease::Lease;
use super::storage::Storage;
use crate::protocol::AcquireRequest;
use crate::protocol::Status;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
/// Approval and adapter are injectable. Production never exposes synthetic approvals over IPC.
pub struct Engine {
    adapter: Arc<dyn Adapter>,
    approval: Arc<dyn Approval>,
    storage: Arc<dyn Storage>,
    leases: Mutex<HashMap<String, Lease>>,
    gate: Mutex<()>,
    pub stopping: AtomicBool,
    pub stop_ready: AtomicBool,
    failures: AtomicU64,
    pending: Mutex<Vec<Lease>>,
    epoch: AtomicU64,
}
impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Engine([redacted])")
    }
}
impl Engine {
    pub fn new(
        adapter: Arc<dyn Adapter>,
        approval: Arc<dyn Approval>,
        storage: Arc<dyn Storage>,
    ) -> Self {
        Self {
            adapter,
            approval,
            storage,
            leases: Mutex::new(HashMap::new()),
            gate: Mutex::new(()),
            stopping: AtomicBool::new(false),
            stop_ready: AtomicBool::new(false),
            failures: AtomicU64::new(0),
            pending: Mutex::new(Vec::new()),
            epoch: AtomicU64::new(0),
        }
    }
    /// # Errors
    ///
    /// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
    pub async fn acquire(
        &self,
        request: AcquireRequest,
        pid: u32,
        cancel: Arc<AtomicBool>,
    ) -> eyre::Result<Grant> {
        request.validate()?;
        let epoch = self.epoch.load(Ordering::SeqCst);
        let _gate = self.gate.lock().await;
        eyre::ensure!(
            !self.stopping.load(Ordering::SeqCst)
                && epoch == self.epoch.load(Ordering::SeqCst)
                && !cancel.load(Ordering::SeqCst),
            "cancelled"
        );
        self.expire().await?;
        let now = crate::protocol::wire::unix_ms()?;
        {
            let leases = self.leases.lock().await;
            if let Some(lease) = leases
                .values()
                .find(|l| l.request == request && l.valid(now))
            {
                return Ok(Grant {
                    id: lease.id.clone(),
                    expires: lease.expires,
                    deadline: lease.deadline,
                    bytes: zeroize::Zeroizing::new(lease.secret.to_vec()),
                });
            }
        }
        let approved = tokio::time::timeout(
            std::time::Duration::from_secs(65),
            self.approval.approve(&request, pid, Arc::clone(&cancel)),
        )
        .await
        .map_err(|_redacted_error| eyre::eyre!("approval timeout"))??;
        eyre::ensure!(approved, "approval denied");
        let approved_at = crate::protocol::wire::unix_ms()?;
        let tick = std::time::Instant::now();
        eyre::ensure!(
            !cancel.load(Ordering::SeqCst)
                && !self.stopping.load(Ordering::SeqCst)
                && epoch == self.epoch.load(Ordering::SeqCst),
            "cancelled"
        );
        let bytes = self.adapter.read(&request).await?;
        let lease = Lease::new(request, bytes, approved_at, tick)?;
        eyre::ensure!(
            lease.valid(crate::protocol::wire::unix_ms()?)
                && !cancel.load(Ordering::SeqCst)
                && !self.stopping.load(Ordering::SeqCst)
                && epoch == self.epoch.load(Ordering::SeqCst),
            "lease expired or cancelled"
        );
        // Registration failures fail closed before the lease enters the visible in-memory cache.
        self.storage.publish(&lease)?;
        if epoch != self.epoch.load(Ordering::SeqCst)
            || cancel.load(Ordering::SeqCst)
            || self.stopping.load(Ordering::SeqCst)
            || !lease.valid(crate::protocol::wire::unix_ms()?)
        {
            self.retire(lease).await;
            eyre::bail!("lease expired or cancelled")
        }
        let grant = Grant {
            id: lease.id.clone(),
            expires: lease.expires,
            deadline: lease.deadline,
            bytes: zeroize::Zeroizing::new(lease.secret.to_vec()),
        };
        let mut leases = self.leases.lock().await;
        if epoch != self.epoch.load(Ordering::SeqCst)
            || self.stopping.load(Ordering::SeqCst)
            || cancel.load(Ordering::SeqCst)
            || !lease.valid(crate::protocol::wire::unix_ms()?)
        {
            drop(leases);
            self.retire(lease).await;
            eyre::bail!("lease expired or cancelled")
        }
        leases.insert(lease.id.clone(), lease);
        Ok(grant)
    }
    async fn retire(&self, mut lease: Lease) {
        if self.storage.remove(&lease).is_err() {
            use zeroize::Zeroize;
            lease.secret.zeroize();
            self.failures.fetch_add(1, Ordering::SeqCst);
            self.pending.lock().await.push(lease);
        }
    }
    /// # Errors
    ///
    /// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
    pub async fn purge(&self, id: Option<&str>) -> eyre::Result<()> {
        if id.is_none() {
            self.epoch.fetch_add(1, Ordering::SeqCst);
        }
        let removed = {
            let mut leases = self.leases.lock().await;
            let ids: Vec<_> = leases
                .keys()
                .filter(|key| id.is_none_or(|id| id == key.as_str()))
                .cloned()
                .collect();
            ids.into_iter()
                .filter_map(|id| leases.remove(&id))
                .collect::<Vec<_>>()
        };
        for lease in removed {
            self.retire(lease).await;
        }
        self.retry_cleanup().await;
        eyre::ensure!(
            self.pending.lock().await.is_empty(),
            "cache cleanup incomplete"
        );
        Ok(())
    }
    /// # Errors
    ///
    /// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
    pub async fn expire(&self) -> eyre::Result<()> {
        let now = crate::protocol::wire::unix_ms()?;
        let expired = {
            let mut leases = self.leases.lock().await;
            let ids: Vec<_> = leases
                .iter()
                .filter(|(_, l)| !l.valid(now))
                .map(|(id, _)| id.clone())
                .collect();
            ids.into_iter()
                .filter_map(|id| leases.remove(&id))
                .collect::<Vec<_>>()
        };
        for lease in expired {
            self.retire(lease).await;
        }
        self.retry_cleanup().await;
        Ok(())
    }
    async fn retry_cleanup(&self) {
        let old = std::mem::take(&mut *self.pending.lock().await);
        for lease in old {
            self.retire(lease).await;
        }
    }
    /// # Errors
    ///
    /// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
    pub async fn status(&self) -> eyre::Result<Status> {
        self.expire().await?;
        Ok(Status {
            running: true,
            active_leases: u64::try_from(self.leases.lock().await.len())?,
            cleanup_failures: self.failures.load(Ordering::SeqCst),
        })
    }
    /// # Errors
    ///
    /// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
    /// Check an opaque live lease without exposing item or secret data.
    /// # Errors
    ///
    /// Returns an error if the lease expired, was purged, or belongs to a former broker instance.
    pub async fn validate_lease(&self, id: &str) -> eyre::Result<crate::protocol::Response> {
        self.expire().await?;
        let leases = self.leases.lock().await;
        let lease = leases
            .get(id)
            .ok_or_else(|| eyre::eyre!("lease unavailable"))?;
        eyre::ensure!(
            lease.valid(crate::protocol::wire::unix_ms()?),
            "lease expired"
        );
        Ok(crate::protocol::Response {
            code: "ok".into(),
            lease_id: Some(lease.id.clone()),
            expires_unix_ms: Some(lease.expires),
            remaining_millis: Some(u64::try_from(
                lease
                    .deadline
                    .saturating_duration_since(std::time::Instant::now())
                    .as_millis(),
            )?),
            status: None,
        })
    }
    /// Stop new grants and purge broker-owned caches.
    /// # Errors
    ///
    /// Reports incomplete persistent cleanup.
    pub async fn shutdown(&self) -> eyre::Result<()> {
        self.stopping.store(true, Ordering::SeqCst);
        self.purge(None).await
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
