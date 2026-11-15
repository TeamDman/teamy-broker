use super::engine_storage::EngineStorage;
use super::lease_delivery::LeaseDelivery;
use super::storage_task_guard::StorageTaskGuard;
use crate::broker::grant::Grant;
use crate::broker::lease::Lease;
use crate::broker::shared_preview::SharedPreview;
use crate::broker::work_guard::WorkGuard;
use crate::protocol::FailureKind;
use crate::protocol::wire;
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::TryRecvError;
use std::time::Duration;
use tokio::sync::OwnedMutexGuard;
use tokio::sync::oneshot::Sender;
use zeroize::Zeroizing;

/// One blocking worker owns publication, delivery acknowledgement and any rollback.
pub(super) struct LeasePublication {
    pub state: Arc<EngineStorage>,
    pub lease: Option<Lease>,
    pub request_work: Option<WorkGuard>,
    pub cancel: Arc<AtomicBool>,
    pub abandoned: Arc<AtomicBool>,
    pub epoch: u64,
    pub provenance: Option<Arc<SharedPreview>>,
    pub _gate: OwnedMutexGuard<()>,
    pub _work: WorkGuard,
    pub _tracked: StorageTaskGuard,
}

impl LeasePublication {
    fn valid(&self, lease: &Lease) -> eyre::Result<()> {
        eyre::ensure!(
            !self.cancel.load(Ordering::SeqCst)
                && !self.abandoned.load(Ordering::SeqCst)
                && !self.state.stopping.load(Ordering::SeqCst)
                && self.epoch == self.state.epoch.load(Ordering::SeqCst),
            FailureKind::RequestCancelled
        );
        eyre::ensure!(lease.valid(wire::unix_ms()?), "lease expired");
        Ok(())
    }

    fn authorized<T>(&self, action: impl FnOnce() -> eyre::Result<T>) -> eyre::Result<T> {
        match &self.provenance {
            Some(shared) => shared.with_publication(action),
            None => action(),
        }
    }

    fn delivery_accepted(&self, receipt: &Receiver<()>, id: &str) -> bool {
        loop {
            match receipt.recv_timeout(Duration::from_millis(20)) {
                Ok(()) => return true,
                Err(RecvTimeoutError::Disconnected) => return false,
                Err(RecvTimeoutError::Timeout) => {
                    // A caller may retain an unpolled future while requesting shutdown.
                    // Drain must not depend on that future acknowledging or being dropped.
                    let leases = self.state.leases.blocking_lock();
                    match receipt.try_recv() {
                        Ok(()) => return true,
                        Err(TryRecvError::Disconnected) => return false,
                        Err(TryRecvError::Empty) => {}
                    }
                    let Some(lease) = leases.get(id) else {
                        return false;
                    };
                    if self.authorized(|| self.valid(lease)).is_err() {
                        return false;
                    }
                }
            }
        }
    }

    pub fn run(mut self, finished: Sender<eyre::Result<LeaseDelivery>>) {
        let Some(lease) = self.lease.take() else {
            let _ = finished.send(Err(FailureKind::CachePublishFailed.into()));
            return;
        };
        let Some(request_work) = self.request_work.take() else {
            let _ = finished.send(Err(FailureKind::CachePublishFailed.into()));
            return;
        };
        let grant = Grant {
            _work: request_work,
            id: lease.id.clone(),
            expires: lease.expires,
            deadline: lease.deadline,
            bytes: Zeroizing::new(lease.secret.to_vec()),
        };
        let mut candidate = Some(lease);
        let mut attempted = false;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let lease = candidate
                .as_ref()
                .ok_or_else(|| eyre::eyre!("publication unavailable"))?;
            self.authorized(|| self.valid(lease))?;
            attempted = true;
            // Native IO can finish after cancellation or revocation. It cannot commit a
            // stale grant: the short final check below either inserts or rolls it back.
            self.state
                .backend
                .publish(lease)
                .map_err(|_redacted| eyre::Report::new(FailureKind::CachePublishFailed))?;
            let mut leases = self.state.leases.blocking_lock();
            self.authorized(|| {
                self.valid(
                    candidate
                        .as_ref()
                        .ok_or_else(|| eyre::eyre!("publication unavailable"))?,
                )?;
                let lease = candidate
                    .take()
                    .ok_or_else(|| eyre::eyre!("publication unavailable"))?;
                leases.insert(lease.id.clone(), lease);
                Ok(())
            })
        }))
        .unwrap_or_else(|_panic| Err(FailureKind::CachePublishFailed.into()));
        if let Err(error) = result {
            if attempted {
                if let Some(lease) = candidate {
                    self.state.remove(lease);
                } else {
                    self.state.rollback(&grant.id);
                }
            }
            let _ = finished.send(Err(error));
            return;
        }
        let id = grant.id.clone();
        let (accepted, receipt) = mpsc::channel();
        let _ = finished.send(Ok(LeaseDelivery {
            grant,
            accepted,
            state: Arc::clone(&self.state),
            cancel: Arc::clone(&self.cancel),
            abandoned: Arc::clone(&self.abandoned),
            epoch: self.epoch,
            provenance: self.provenance.clone(),
        }));
        // No Drop implementation starts work. A dropped future or channel output closes
        // this receipt and the already tracked worker cleans its exact generation.
        if !self.delivery_accepted(&receipt, &id) {
            self.state.rollback(&id);
        }
    }
}
