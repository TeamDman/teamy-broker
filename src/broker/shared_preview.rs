use crate::protocol::ApprovalPreviewDisposition;
use crate::protocol::SharedPreviewSnapshot;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
/// One resident preview. No credential queue, provider, storage, or lease is reachable here.
#[derive(Debug, Default)]
pub struct SharedPreviewHub {
    current: Mutex<Option<Arc<SharedPreview>>>,
}
pub struct SharedPreview {
    snapshot: SharedPreviewSnapshot,
    deadline: Mutex<Instant>,
    authorization: Option<Arc<super::credential_grant::CredentialGrant>>,
    resolved_at: Mutex<Option<(u64, Instant)>>,
    cancel: Arc<AtomicBool>,
    outcome: Mutex<Option<ApprovalPreviewDisposition>>,
    supplied: Mutex<Option<zeroize::Zeroizing<Vec<u8>>>>,
    pub provider_requested: AtomicBool,
    pub provider_failed: AtomicBool,
}
impl SharedPreviewHub {
    pub fn resident() -> &'static Self {
        static HUB: OnceLock<SharedPreviewHub> = OnceLock::new();
        HUB.get_or_init(Self::default)
    }
    /// # Errors
    /// Rejects a simultaneous preview or an invalid clock.
    /// # Panics
    /// Panics if another thread poisoned the internal state mutex.
    pub fn begin(&self, cancel: Arc<AtomicBool>) -> eyre::Result<Arc<SharedPreview>> {
        self.begin_context(cancel, None, None)
    }
    /// # Errors
    /// Rejects invalid metadata, unavailable state, cancellation or expiry.
    pub fn begin_credential(
        &self,
        cancel: Arc<AtomicBool>,
        context: crate::protocol::credential_context::CredentialContext,
    ) -> eyre::Result<Arc<SharedPreview>> {
        context.request.validate()?;
        self.begin_context(cancel, Some(context), None)
    }
    pub(crate) fn begin_bound(
        &self,
        cancel: Arc<AtomicBool>,
        context: crate::protocol::credential_context::CredentialContext,
        authorization: Arc<super::credential_grant::CredentialGrant>,
    ) -> eyre::Result<Arc<SharedPreview>> {
        self.begin_context(cancel, Some(context), Some(authorization))
    }
    fn begin_context(
        &self,
        cancel: Arc<AtomicBool>,
        credential: Option<crate::protocol::credential_context::CredentialContext>,
        authorization: Option<Arc<super::credential_grant::CredentialGrant>>,
    ) -> eyre::Result<Arc<SharedPreview>> {
        let mut current = self.current.lock().expect("preview hub lock");
        eyre::ensure!(
            current
                .as_ref()
                .is_none_or(|request| request.outcome().is_some()),
            "preview busy"
        );
        let request = Arc::new(SharedPreview {
            snapshot: SharedPreviewSnapshot {
                kind: if credential.is_some() {
                    "credential-use-v1"
                } else {
                    "no-vault-preview-v1"
                }
                .into(),
                credential: credential.map(Box::new),
                request_id: uuid::Uuid::new_v4().to_string(),
                expires_unix_ms: crate::protocol::wire::unix_ms()?
                    .checked_add(60_000)
                    .ok_or_else(|| eyre::eyre!("invalid clock"))?,
            },
            deadline: Mutex::new(Instant::now() + Duration::from_mins(1)),
            authorization,
            resolved_at: Mutex::new(None),
            cancel,
            outcome: Mutex::new(None),
            supplied: Mutex::new(None),
            provider_requested: AtomicBool::new(false),
            provider_failed: AtomicBool::new(false),
        });
        *current = Some(Arc::clone(&request));
        Ok(request)
    }
    /// # Panics
    /// Panics if another thread poisoned the internal state mutex.
    pub fn snapshot(&self) -> Option<SharedPreviewSnapshot> {
        let current = self.current.lock().expect("preview hub lock");
        current
            .as_ref()
            .filter(|request| request.outcome().is_none())
            .map(|request| {
                let mut snapshot = request.snapshot.clone();
                if let Some(c) = snapshot.credential.as_mut() {
                    c.provider_state = if request.provider_failed.load(Ordering::Acquire) {
                        "failed"
                    } else if request.provider_requested.load(Ordering::Acquire) {
                        "waiting"
                    } else {
                        "idle"
                    }
                    .into();
                }
                snapshot
            })
    }
    /// # Errors
    /// Rejects a stale identity, expiry, cancellation, or a second decision.
    /// # Panics
    /// Panics if another thread poisoned the internal state mutex.
    /// # Panics
    /// Panics if approval state mutexes are poisoned.
    pub fn supply(&self, id: &str, bytes: zeroize::Zeroizing<Vec<u8>>) -> bool {
        self.current
            .lock()
            .expect("approval hub lock")
            .as_ref()
            .is_some_and(|r| r.snapshot.request_id == id && r.supply(bytes))
    }
    /// # Panics
    /// Panics if approval state mutexes are poisoned.
    pub fn request_provider(&self, id: &str) -> bool {
        self.current
            .lock()
            .expect("approval hub lock")
            .as_ref()
            .is_some_and(|r| r.snapshot.request_id == id && r.request_provider())
    }
    /// # Panics
    /// Panics if approval state mutexes are poisoned.
    pub fn cancel_current(&self) {
        if let Some(r) = self.current.lock().expect("approval hub lock").as_ref() {
            r.invalidate();
        }
    }
    /// # Errors
    /// Rejects stale, expired or already resolved requests.
    /// # Panics
    /// Panics on a poisoned approval mutex.
    pub fn decide(&self, id: &str, disposition: ApprovalPreviewDisposition) -> eyre::Result<()> {
        let current = self.current.lock().expect("preview hub lock");
        let request = current
            .as_ref()
            .ok_or_else(|| eyre::eyre!("preview unavailable"))?;
        eyre::ensure!(
            request.snapshot.request_id == id && request.resolve(disposition),
            "preview unavailable"
        );
        Ok(())
    }
}
impl std::fmt::Debug for SharedPreview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SharedApproval([redacted])")
    }
}
impl SharedPreview {
    fn terminal(&self) -> Option<ApprovalPreviewDisposition> {
        if self.cancel.load(Ordering::SeqCst) {
            Some(ApprovalPreviewDisposition::Cancelled)
        } else if Instant::now() >= *self.deadline.lock().expect("deadline lock")
            || crate::protocol::wire::unix_ms()
                .is_ok_and(|now| now >= self.snapshot.expires_unix_ms)
        {
            Some(ApprovalPreviewDisposition::TimedOut)
        } else {
            None
        }
    }
    /// # Panics
    /// Panics if another thread poisoned the internal state mutex.
    pub fn outcome(&self) -> Option<ApprovalPreviewDisposition> {
        let mut outcome = self.outcome.lock().expect("preview outcome lock");
        if outcome.is_none() {
            *outcome = self.terminal();
        }
        *outcome
    }
    /// The mutex serializes every device decision with observed cancellation and hard expiry.
    /// # Panics
    /// Panics if another thread poisoned the internal state mutex.
    #[must_use]
    pub fn is_credential(&self) -> bool {
        self.snapshot.credential.is_some()
    }
    #[must_use]
    pub(crate) fn bound_to(&self, grant: &Arc<super::credential_grant::CredentialGrant>) -> bool {
        self.authorization
            .as_ref()
            .is_some_and(|bound| Arc::ptr_eq(bound, grant))
    }
    #[must_use]
    pub fn synthetic_fixture(&self) -> bool {
        self.snapshot
            .credential
            .as_ref()
            .is_some_and(|c| c.synthetic_fixture)
    }
    #[must_use]
    pub fn allows_supply(&self) -> bool {
        self.snapshot.credential.as_ref().is_some_and(|c| c.supply)
    }
    /// # Panics
    /// Panics if approval state mutexes are poisoned.
    pub fn request_provider(&self) -> bool {
        let mut outcome = self.outcome.lock().expect("approval outcome lock");
        if outcome.is_none() {
            *outcome = self.terminal();
        }
        outcome.is_none()
            && self.snapshot.credential.is_some()
            && !self.provider_failed.load(Ordering::Acquire)
            && !self.provider_requested.swap(true, Ordering::AcqRel)
    }
    /// # Panics
    /// Panics if approval state mutexes are poisoned.
    pub fn provider_value(&self, bytes: zeroize::Zeroizing<Vec<u8>>) -> bool {
        self.publish_value(bytes, false)
    }
    /// # Panics
    /// Panics if approval state mutexes are poisoned.
    pub fn supply(&self, bytes: zeroize::Zeroizing<Vec<u8>>) -> bool {
        if self.synthetic_fixture()
            && bytes.as_slice() != super::synthetic_credential_fixture::INPUT
        {
            return false;
        }
        self.publish_value(bytes, true)
    }
    fn publish_value(&self, bytes: zeroize::Zeroizing<Vec<u8>>, require_supply: bool) -> bool {
        let mut outcome = self.outcome.lock().expect("approval outcome lock");
        if outcome.is_none() {
            *outcome = self.terminal();
        }
        if outcome.is_some()
            || bytes.is_empty()
            || bytes.len() > crate::protocol::MAX_SECRET
            || !self
                .snapshot
                .credential
                .as_ref()
                .is_some_and(|c| !require_supply || c.supply)
        {
            return false;
        }
        let Ok(now) = crate::protocol::wire::unix_ms() else {
            return false;
        };
        *self.resolved_at.lock().expect("resolution lock") = Some((now, Instant::now()));
        *self.supplied.lock().expect("approval secret lock") = Some(bytes);
        *outcome = Some(ApprovalPreviewDisposition::ApprovedTest);
        true
    }
    /// # Panics
    /// Panics if approval state mutexes are poisoned.
    #[cfg(test)]
    pub(crate) fn expire_original_deadline(&self) {
        *self.deadline.lock().unwrap() = Instant::now()
            .checked_sub(Duration::from_millis(1))
            .unwrap();
    }
    pub(crate) fn invalidate(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        self.resolve(ApprovalPreviewDisposition::Cancelled);
        self.clear_value();
    }
    pub(crate) fn resolution_time(&self) -> eyre::Result<(u64, Instant)> {
        self.resolved_at
            .lock()
            .map_err(|_redacted| eyre::eyre!("resolution unavailable"))?
            .ok_or_else(|| eyre::eyre!("resolution unavailable"))
    }
    pub(crate) fn publication_valid(&self) -> eyre::Result<()> {
        eyre::ensure!(
            !self.cancel.load(Ordering::SeqCst),
            crate::protocol::FailureKind::RequestCancelled
        );
        eyre::ensure!(
            Instant::now() < *self.deadline.lock().expect("deadline lock")
                && crate::protocol::wire::unix_ms()? < self.snapshot.expires_unix_ms,
            crate::protocol::FailureKind::ApprovalTimedOut
        );
        eyre::ensure!(
            self.outcome() == Some(ApprovalPreviewDisposition::ApprovedTest),
            crate::protocol::FailureKind::ApprovalUnavailable
        );
        Ok(())
    }
    pub(crate) fn with_publication<T>(
        &self,
        action: impl FnOnce() -> eyre::Result<T>,
    ) -> eyre::Result<T> {
        let checked = || {
            self.publication_valid()?;
            action()
        };
        match &self.authorization {
            Some(grant) => grant.with_valid(checked),
            None => checked(),
        }
    }
    pub(crate) fn clear_value(&self) {
        self.supplied.lock().expect("approval value lock").take();
    }
    /// # Panics
    /// Panics if the approval value mutex is poisoned.
    pub fn take_supply(&self) -> Option<zeroize::Zeroizing<Vec<u8>>> {
        self.supplied.lock().expect("approval secret lock").take()
    }
    /// # Errors
    /// Rejects invalid metadata, unavailable state, cancellation or expiry.
    /// # Panics
    /// Panics on a poisoned approval mutex.
    pub fn resolve(&self, disposition: ApprovalPreviewDisposition) -> bool {
        if !matches!(
            disposition,
            ApprovalPreviewDisposition::ApprovedTest
                | ApprovalPreviewDisposition::DeniedTest
                | ApprovalPreviewDisposition::Cancelled
                | ApprovalPreviewDisposition::TimedOut
                | ApprovalPreviewDisposition::Unavailable
        ) {
            return false;
        }
        if self.is_credential() && disposition == ApprovalPreviewDisposition::ApprovedTest {
            return false;
        }
        let mut outcome = self.outcome.lock().expect("preview outcome lock");
        if outcome.is_none() {
            *outcome = self.terminal();
        }
        if outcome.is_some() {
            return false;
        }
        *outcome = Some(disposition);
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn winner_cannot_publish_after_original_wall_deadline() {
        let hub = SharedPreviewHub::default();
        let mut request = hub.begin(Arc::new(AtomicBool::new(false))).unwrap();
        assert!(request.resolve(ApprovalPreviewDisposition::ApprovedTest));
        drop(hub);
        Arc::get_mut(&mut request).unwrap().snapshot.expires_unix_ms = 0;
        assert!(Instant::now() < *request.deadline.lock().unwrap());
        let mut called = false;
        assert!(
            request
                .with_publication(|| {
                    called = true;
                    Ok(())
                })
                .is_err()
        );
        assert!(!called);
    }
    #[test]
    fn simultaneous_devices_have_one_winner_and_stale_ids_fail() {
        let hub = Arc::new(SharedPreviewHub::default());
        let request = hub.begin(Arc::new(AtomicBool::new(false))).unwrap();
        let id = hub.snapshot().unwrap().request_id;
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let handles: Vec<_> = [
            ApprovalPreviewDisposition::ApprovedTest,
            ApprovalPreviewDisposition::DeniedTest,
        ]
        .into_iter()
        .map(|decision| {
            let hub = Arc::clone(&hub);
            let id = id.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                hub.decide(&id, decision).is_ok()
            })
        })
        .collect();
        barrier.wait();
        assert_eq!(
            handles
                .into_iter()
                .filter_map(|handle| handle.join().ok())
                .filter(|won| *won)
                .count(),
            1
        );
        assert!(request.outcome().is_some());
        assert!(hub.snapshot().is_none());
        let next = hub.begin(Arc::new(AtomicBool::new(false))).unwrap();
        assert!(
            hub.decide(&id, ApprovalPreviewDisposition::ApprovedTest)
                .is_err()
        );
        assert!(next.outcome().is_none());
    }
    #[test]
    fn disconnect_cancellation_and_expiry_reject_late_approval() {
        let hub = SharedPreviewHub::default();
        let cancel = Arc::new(AtomicBool::new(false));
        let request = hub.begin(Arc::clone(&cancel)).unwrap();
        let id = hub.snapshot().unwrap().request_id;
        cancel.store(true, Ordering::SeqCst);
        assert!(
            hub.decide(&id, ApprovalPreviewDisposition::ApprovedTest)
                .is_err()
        );
        assert_eq!(
            request.outcome(),
            Some(ApprovalPreviewDisposition::Cancelled)
        );
        assert!(hub.snapshot().is_none());
        let expired = SharedPreview {
            snapshot: SharedPreviewSnapshot {
                kind: "no-vault-preview-v1".into(),
                credential: None,
                request_id: id,
                expires_unix_ms: 0,
            },
            deadline: Mutex::new(Instant::now()),
            authorization: None,
            resolved_at: Mutex::new(None),
            cancel: Arc::new(AtomicBool::new(false)),
            outcome: Mutex::new(None),
            supplied: Mutex::new(None),
            provider_requested: AtomicBool::new(false),
            provider_failed: AtomicBool::new(false),
        };
        *expired.deadline.lock().unwrap() = Instant::now()
            .checked_sub(Duration::from_millis(1))
            .unwrap();
        assert!(!expired.resolve(ApprovalPreviewDisposition::ApprovedTest));
        assert_eq!(
            expired.outcome(),
            Some(ApprovalPreviewDisposition::TimedOut)
        );
    }
    #[test]
    fn close_is_local_and_process_replacement_has_no_pending_state() {
        let hub = SharedPreviewHub::default();
        let request = hub.begin(Arc::new(AtomicBool::new(false))).unwrap();
        assert!(!request.resolve(ApprovalPreviewDisposition::Closed));
        assert!(hub.snapshot().is_some());
        assert!(SharedPreviewHub::default().snapshot().is_none());
        assert!(request.resolve(ApprovalPreviewDisposition::DeniedTest));
        assert!(!request.resolve(ApprovalPreviewDisposition::ApprovedTest));
    }
}
