use super::adapter::Adapter;
use super::approval::Approval;
use super::grant::Grant;
use super::lease::Lease;
use super::storage::Storage;
use crate::protocol::AcquireRequest;
use crate::protocol::FailureKind;
use crate::protocol::Status;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
/// Approval and adapter are injectable. The explicit preview never enters acquisition.
pub struct Engine {
    activity: Arc<super::work_state::WorkState>,
    adapter: Arc<dyn Adapter>,
    approval: Arc<dyn Approval>,
    storage: Arc<dyn Storage>,
    pub(crate) leases: Mutex<HashMap<String, Lease>>,
    #[cfg(test)]
    pub(crate) fixture_controller: std::sync::Mutex<Option<Arc<super::phone::Controller>>>,
    gate: Mutex<()>,
    pub stopping: AtomicBool,
    pub stop_ready: AtomicBool,
    failures: AtomicU64,
    pending: Mutex<Vec<Lease>>,
    epoch: AtomicU64,
    #[cfg(test)]
    pub(crate) publication_gate:
        std::sync::Mutex<Option<Arc<super::publication_gate::PublicationGate>>>,
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
            activity: Arc::default(),
            adapter,
            approval,
            storage,
            leases: Mutex::new(HashMap::new()),
            #[cfg(test)]
            fixture_controller: std::sync::Mutex::new(None),
            gate: Mutex::new(()),
            stopping: AtomicBool::new(false),
            stop_ready: AtomicBool::new(false),
            failures: AtomicU64::new(0),
            pending: Mutex::new(Vec::new()),
            epoch: AtomicU64::new(0),
            #[cfg(test)]
            publication_gate: std::sync::Mutex::new(None),
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
        eyre::ensure!(
            request != super::synthetic_credential_fixture::request(),
            "typed fixture acquisition required"
        );
        self.acquire_inner(request, pid, cancel, None).await
    }
    /// # Errors
    /// Requires one exact live fixture grant; no cache lookup, fallback approval or live provider.
    pub async fn acquire_fixture(
        &self,
        request: AcquireRequest,
        pid: u32,
        cancel: Arc<AtomicBool>,
    ) -> eyre::Result<Grant> {
        super::synthetic_credential_fixture::validate(&request)?;
        #[cfg(test)]
        let injected = self
            .fixture_controller
            .lock()
            .map_err(|_redacted| eyre::eyre!("fixture controller unavailable"))?
            .clone();
        #[cfg(test)]
        let controller = match injected {
            Some(value) => value,
            None => super::phone::Controller::resident()?,
        };
        #[cfg(not(test))]
        let controller = super::phone::Controller::resident()?;
        self.acquire_inner(request, pid, cancel, Some(&controller))
            .await
    }
    async fn acquire_inner(
        &self,
        request: AcquireRequest,
        pid: u32,
        cancel: Arc<AtomicBool>,
        fixture: Option<&Arc<super::phone::Controller>>,
    ) -> eyre::Result<Grant> {
        request.validate()?;
        let request_work = self.activity.request()?;
        let queued = request_work.phase(super::work_kind::WorkKind::Queued);
        let epoch = self.epoch.load(Ordering::SeqCst);
        let _gate = self.gate.lock().await;
        drop(queued);
        eyre::ensure!(
            !self.stopping.load(Ordering::SeqCst)
                && epoch == self.epoch.load(Ordering::SeqCst)
                && !cancel.load(Ordering::SeqCst),
            "cancelled"
        );
        let fixture_grant = fixture
            .map(|controller| controller.claim_fixture_permission(&request))
            .transpose()?;
        self.expire().await?;
        let now = crate::protocol::wire::unix_ms()?;
        if fixture_grant.is_none() {
            let leases = self.leases.lock().await;
            if let Some(lease) = leases
                .values()
                .find(|l| l.request == request && l.valid(now))
            {
                return Ok(Grant {
                    work: request_work,
                    id: lease.id.clone(),
                    expires: lease.expires,
                    deadline: lease.deadline,
                    bytes: zeroize::Zeroizing::new(lease.secret.to_vec()),
                });
            }
        }
        let approved = if let Some(grant) = &fixture_grant {
            let approved = tokio::time::timeout(
                std::time::Duration::from_secs(65),
                self.approval.fixture_decision(
                    &request,
                    pid,
                    Arc::clone(&cancel),
                    request_work.phase(super::work_kind::WorkKind::Approval),
                    Arc::clone(grant),
                ),
            )
            .await
            .map_err(|_redacted| FailureKind::ApprovalTimedOut)??;
            eyre::ensure!(
                matches!(&approved,super::approval_decision::ApprovalDecision::Shared(shared) if shared.synthetic_fixture()&&shared.bound_to(grant)),
                "bound fixture approval required"
            );
            approved
        } else {
            self.resolve_approval(&request, pid, Arc::clone(&cancel), &request_work)
                .await?
        };
        if cancel.load(Ordering::SeqCst)
            || self.stopping.load(Ordering::SeqCst)
            || epoch != self.epoch.load(Ordering::SeqCst)
        {
            if let super::approval_decision::ApprovalDecision::Shared(shared) = &approved {
                shared.resolve(crate::protocol::ApprovalPreviewDisposition::Cancelled);
                shared.clear_value();
            }
            return Err(FailureKind::RequestCancelled.into());
        }
        let provenance = match &approved {
            super::approval_decision::ApprovalDecision::Shared(shared) => Some(Arc::clone(shared)),
            _ => None,
        };
        let (bytes, approved_at, tick) = self
            .read_approved(approved, &request, &request_work)
            .await?;
        let lease = Lease::new(request, bytes, approved_at, tick)?;
        eyre::ensure!(
            lease.valid(crate::protocol::wire::unix_ms()?)
                && !cancel.load(Ordering::SeqCst)
                && !self.stopping.load(Ordering::SeqCst)
                && epoch == self.epoch.load(Ordering::SeqCst),
            "lease expired or cancelled"
        );
        self.publish_lease(lease, request_work, cancel, epoch, provenance)
            .await
    }
    async fn publish_lease(
        &self,
        lease: Lease,
        request_work: super::work_guard::WorkGuard,
        cancel: Arc<AtomicBool>,
        epoch: u64,
        provenance: Option<Arc<super::shared_preview::SharedPreview>>,
    ) -> eyre::Result<Grant> {
        let grant = Grant {
            work: request_work,
            id: lease.id.clone(),
            expires: lease.expires,
            deadline: lease.deadline,
            bytes: zeroize::Zeroizing::new(lease.secret.to_vec()),
        };
        let _storage = grant.work.phase(super::work_kind::WorkKind::Storage);
        #[cfg(test)]
        let test_gate = self.publication_gate.lock().unwrap().clone();
        #[cfg(test)]
        if let Some(gate) = test_gate {
            gate.ready.notify_one();
            gate.release.notified().await;
        }
        let mut leases = self.leases.lock().await;
        let mut candidate = Some(lease);
        let mut published = false;
        let valid = || {
            eyre::ensure!(
                !cancel.load(Ordering::SeqCst)
                    && !self.stopping.load(Ordering::SeqCst)
                    && epoch == self.epoch.load(Ordering::SeqCst),
                FailureKind::RequestCancelled
            );
            if let Some(shared) = &provenance {
                shared.publication_valid()?;
            }
            Ok::<_, eyre::Report>(())
        };
        let mut action = || {
            valid()?;
            let lease = candidate
                .as_ref()
                .ok_or_else(|| eyre::eyre!("publication unavailable"))?;
            eyre::ensure!(
                lease.valid(crate::protocol::wire::unix_ms()?),
                "lease expired"
            );
            self.storage
                .publish(lease)
                .map_err(|_redacted| eyre::Report::new(FailureKind::CachePublishFailed))?;
            published = true;
            valid()?;
            eyre::ensure!(
                lease.valid(crate::protocol::wire::unix_ms()?),
                "lease expired"
            );
            leases.insert(
                lease.id.clone(),
                candidate
                    .take()
                    .ok_or_else(|| eyre::eyre!("publication unavailable"))?,
            );
            Ok(())
        };
        let result = match &provenance {
            Some(shared) => shared.with_publication(action),
            None => action(),
        };
        drop(leases);
        if result.is_err()
            && published
            && let Some(lease) = candidate.take()
        {
            self.retire(lease).await;
        }
        result?;
        Ok(grant)
    }
    async fn resolve_approval(
        &self,
        request: &AcquireRequest,
        pid: u32,
        cancel: Arc<AtomicBool>,
        request_work: &super::work_guard::WorkGuard,
    ) -> eyre::Result<super::approval_decision::ApprovalDecision> {
        let approved = tokio::time::timeout(
            std::time::Duration::from_secs(65),
            self.approval.decision(
                request,
                pid,
                Arc::clone(&cancel),
                request_work.phase(super::work_kind::WorkKind::Approval),
            ),
        )
        .await
        .map_err(|_redacted_error| eyre::Report::new(FailureKind::ApprovalTimedOut))?
        .map_err(|error| {
            eyre::Report::new(
                error
                    .downcast_ref::<FailureKind>()
                    .copied()
                    .unwrap_or(FailureKind::ApprovalUnavailable),
            )
        })?;
        eyre::ensure!(
            !matches!(approved, super::approval_decision::ApprovalDecision::Denied),
            FailureKind::UserDenied
        );
        Ok(approved)
    }
    async fn read_approved(
        &self,
        approved: super::approval_decision::ApprovalDecision,
        request: &AcquireRequest,
        request_work: &super::work_guard::WorkGuard,
    ) -> eyre::Result<(zeroize::Zeroizing<Vec<u8>>, u64, std::time::Instant)> {
        let mut approved_at = crate::protocol::wire::unix_ms()?;
        let mut tick = std::time::Instant::now();
        let bytes = match approved {
            super::approval_decision::ApprovalDecision::DesktopRead => {
                self.read_upstream(request, request_work).await?
            }
            super::approval_decision::ApprovalDecision::Shared(shared) => {
                let bytes = super::credential_approvals::resolve(Arc::clone(&shared), async {
                    if shared.synthetic_fixture() {
                        super::synthetic_credential_fixture::read(request)
                    } else {
                        self.read_upstream(request, request_work).await
                    }
                })
                .await?;
                (approved_at, tick) = shared.resolution_time()?;
                bytes
            }
            super::approval_decision::ApprovalDecision::Supplied(bytes) => {
                eyre::ensure!(
                    !bytes.is_empty() && bytes.len() <= crate::protocol::MAX_SECRET,
                    "supplied value rejected"
                );
                bytes
            }
            super::approval_decision::ApprovalDecision::Denied => {
                return Err(FailureKind::UserDenied.into());
            }
        };
        Ok((bytes, approved_at, tick))
    }
    async fn read_upstream(
        &self,
        request: &AcquireRequest,
        work: &super::work_guard::WorkGuard,
    ) -> eyre::Result<zeroize::Zeroizing<Vec<u8>>> {
        let _work = work.phase(super::work_kind::WorkKind::Upstream);
        self.adapter
            .read(request)
            .await
            .map_err(|_redacted_error| FailureKind::UpstreamReadFailed.into())
    }
    pub(crate) fn control_work(&self) -> eyre::Result<super::work_guard::WorkGuard> {
        self.activity.request()
    }
    /// Preview only the resident approval UI. No adapter, cache, expiry or storage operation.
    /// # Errors
    /// Fails on busy/stopping state, cancellation, timeout or unavailable preview implementation.
    pub async fn approval_preview(
        &self,
        pid: u32,
        cancel: Arc<AtomicBool>,
    ) -> eyre::Result<crate::protocol::ApprovalPreviewResult> {
        let request_work = self.activity.request()?;
        let _gate = self
            .gate
            .try_lock()
            .map_err(|_busy| eyre::Report::new(FailureKind::ShutdownBlocked))?;
        let counts = self.activity.snapshot().counts;
        eyre::ensure!(
            counts[super::work_kind::WorkKind::Approval as usize] == 0
                && counts[super::work_kind::WorkKind::Dialog as usize] == 0,
            FailureKind::ShutdownBlocked
        );
        eyre::ensure!(
            !self.stopping.load(Ordering::SeqCst) && !cancel.load(Ordering::SeqCst),
            FailureKind::RequestCancelled
        );
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(65),
            self.approval.preview(
                pid,
                Arc::clone(&cancel),
                request_work.phase(super::work_kind::WorkKind::Approval),
            ),
        )
        .await;
        match result {
            Ok(result) => result,
            Err(_timeout) => {
                cancel.store(true, Ordering::SeqCst);
                Err(FailureKind::ApprovalTimedOut.into())
            }
        }
    }
    async fn retire(&self, mut lease: Lease) {
        let _work = self.activity.phase(super::work_kind::WorkKind::Storage);
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
        let _work = self.activity.phase(super::work_kind::WorkKind::Storage);
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
        let _work = self.activity.phase(super::work_kind::WorkKind::Storage);
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
        use super::work_kind::WorkKind;
        use crate::protocol::stop_reason::StopReason;
        let leases = self.leases.lock().await;
        let pending = self.pending.lock().await;
        let work = self.activity.snapshot();
        let now = crate::protocol::wire::unix_ms()?;
        let active_leases =
            u64::try_from(leases.values().filter(|lease| lease.valid(now)).count())?;
        let pending_cleanup = u64::try_from(pending.len())?;
        let busy = work.counts.iter().any(|count| *count != 0);
        let mut reasons = Vec::new();
        if busy {
            reasons.push(StopReason::ActiveOperations);
        }
        if active_leases != 0 {
            reasons.push(StopReason::CachedLeases);
        }
        if pending_cleanup != 0 {
            reasons.push(StopReason::PendingCleanup);
        }
        if work.stopping {
            reasons.push(StopReason::AlreadyStopping);
        }
        Ok(Status {
            running: true,
            shutdown_state_available: true,
            active_leases,
            cleanup_failures: self.failures.load(Ordering::SeqCst),
            active_requests: work.counts[WorkKind::Request as usize],
            queued_requests: work.counts[WorkKind::Queued as usize],
            active_approvals: work.counts[WorkKind::Approval as usize],
            pending_approvals: work.counts[WorkKind::Approval as usize]
                .saturating_sub(work.counts[WorkKind::Dialog as usize]),
            open_approval_dialogs: work.counts[WorkKind::Dialog as usize],
            upstream_reads: work.counts[WorkKind::Upstream as usize],
            storage_operations: work.counts[WorkKind::Storage as usize],
            pending_cleanup,
            stopping: work.stopping,
            would_interrupt_operations: busy,
            would_discard_cached_leases: active_leases != 0,
            safe_to_stop: reasons.is_empty(),
            stop_reasons: reasons,
        })
    }
    /// Recheck current work and close admission atomically before shutdown.
    /// # Errors
    /// Refuses unsafe shutdown unless force explicitly authorizes interruption and lease discard.
    pub async fn stop(&self, force: bool) -> eyre::Result<()> {
        let leases = self.leases.lock().await;
        let pending = self.pending.lock().await;
        let now = crate::protocol::wire::unix_ms()?;
        self.activity.admit_stop(
            force,
            leases.values().any(|lease| lease.valid(now)),
            !pending.is_empty(),
        )?;
        self.stopping.store(true, Ordering::SeqCst);
        drop(pending);
        drop(leases);
        self.shutdown().await
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
            failure: None,
        })
    }
    /// Stop new grants and purge broker-owned caches.
    /// # Errors
    ///
    /// Reports incomplete persistent cleanup.
    pub async fn shutdown(&self) -> eyre::Result<()> {
        self.activity.close();
        self.stopping.store(true, Ordering::SeqCst);
        self.purge(None).await
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
pub(crate) mod tests;
