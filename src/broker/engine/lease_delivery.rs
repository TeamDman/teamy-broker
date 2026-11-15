use super::engine_storage::EngineStorage;
use crate::broker::grant::Grant;
use crate::broker::shared_preview::SharedPreview;
use crate::protocol::FailureKind;
use crate::protocol::wire;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;

/// Dropping an unreceived delivery closes the acknowledgement; its worker rolls back.
pub(super) struct LeaseDelivery {
    pub grant: Grant,
    pub accepted: Sender<()>,
    pub state: Arc<EngineStorage>,
    pub cancel: Arc<AtomicBool>,
    pub abandoned: Arc<AtomicBool>,
    pub epoch: u64,
    pub provenance: Option<Arc<SharedPreview>>,
}

impl LeaseDelivery {
    pub async fn accept(self) -> eyre::Result<Grant> {
        let leases = self.state.leases.lock().await;
        let accept = || {
            eyre::ensure!(
                !self.cancel.load(Ordering::SeqCst)
                    && !self.abandoned.load(Ordering::SeqCst)
                    && !self.state.stopping.load(Ordering::SeqCst)
                    && self.epoch == self.state.epoch.load(Ordering::SeqCst),
                FailureKind::RequestCancelled
            );
            let now = wire::unix_ms()?;
            eyre::ensure!(
                leases
                    .get(&self.grant.id)
                    .is_some_and(|lease| lease.valid(now)),
                "lease expired or purged"
            );
            self.accepted
                .send(())
                .map_err(|_closed| FailureKind::CachePublishFailed)?;
            Ok(())
        };
        match &self.provenance {
            Some(shared) => shared.with_publication(accept),
            None => accept(),
        }?;
        drop(leases);
        Ok(self.grant)
    }
}
