use super::ENROLLMENT_WINDOW;
use super::SCOPE;
use super::config::PhoneProfile;
use super::identity;
use super::pairing_confirmation;
use super::preview_hub::PreviewHub;
use super::store::Record;
use super::store::Store;
use crate::broker::credential_approvals;
use crate::broker::credential_grant::CredentialGrant;
use crate::broker::credential_permission::CredentialPermission;
use crate::broker::shared_preview::SharedPreviewHub;
use crate::broker::synthetic_credential_fixture;
use crate::paths::AppHome;
use crate::protocol::AcquireRequest;
use crate::protocol::ApprovalPreviewDisposition;
use crate::protocol::PhoneStatus;
use crate::protocol::SharedPreviewSnapshot;
use crate::protocol::phone_status::PhoneConnectionState;
use crate::protocol::phone_status::PhoneDiagnostic;
use crate::protocol::phone_status::PhoneRuntimeState;
use crate::protocol::wire;
#[cfg(test)]
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;
use std::time::Instant;

#[derive(Debug)]
pub struct Controller {
    pub(super) store: Store,
    pub(super) hub: PreviewHub,
    pub(super) credential_hub: PreviewHub,
    pub(super) record: Mutex<Option<Record>>,
    pub(super) pair_until: Mutex<Option<Instant>>,
    pub(super) confirmation: Mutex<Option<Arc<pairing_confirmation::Attempt>>>,
    pub(super) permission: Mutex<Option<CredentialPermission>>,
    pub(super) connected: AtomicBool,
    pub(super) listener: AtomicBool,
    pub(super) runtime: AtomicU8,
    pub(super) actor_active: AtomicBool,
    pub(super) diagnostic: Mutex<PhoneDiagnostic>,
}
impl Controller {
    /// # Errors
    /// Rejects unavailable protected state, invalid scope or conflicting relationship changes.
    pub fn resident() -> eyre::Result<Arc<Self>> {
        static INSTANCE: OnceLock<Arc<Controller>> = OnceLock::new();
        if let Some(value) = INSTANCE.get() {
            return Ok(Arc::clone(value));
        }
        let store = Store::current_user()?;
        let record = store.load()?;
        let value = Arc::new(Self {
            store,
            hub: PreviewHub::Resident,
            credential_hub: PreviewHub::Resident,
            record: Mutex::new(record),
            pair_until: Mutex::new(None),
            confirmation: Mutex::new(None),
            permission: Mutex::new(None),
            connected: AtomicBool::new(false),
            listener: AtomicBool::new(false),
            runtime: AtomicU8::new(0),
            actor_active: AtomicBool::new(false),
            diagnostic: Mutex::new(PhoneDiagnostic::default()),
        });
        let _ = INSTANCE.set(Arc::clone(&value));
        Ok(Arc::clone(INSTANCE.get().unwrap_or(&value)))
    }
    /// # Errors
    /// Rejects unavailable protected state, invalid scope or conflicting relationship changes.
    pub fn status(&self) -> eyre::Result<PhoneStatus> {
        Ok(PhoneStatus {
            paired: self
                .record
                .lock()
                .map_err(|_redacted_error| eyre::eyre!("phone state unavailable"))?
                .as_ref()
                .is_some_and(|r| r.phone_certificate.is_some()),
            pairing_window_open: self.pairing_live()?,
            pairing_attempt_id: self
                .confirmation
                .lock()
                .map_err(|_redacted_error| eyre::eyre!("pairing unavailable"))?
                .as_ref()
                .filter(|a| !a.finished())
                .map(|a| a.id.clone()),
            listener: self.listener.load(Ordering::Acquire),
            connection: if self.connected.load(Ordering::Acquire) {
                PhoneConnectionState::Connected
            } else {
                PhoneConnectionState::Disconnected
            },
            runtime: self.runtime_status(),
            diagnostic: *self
                .diagnostic
                .lock()
                .map_err(|_redacted| eyre::eyre!("phone diagnostics unavailable"))?,
            scope: SCOPE.into(),
            peer: "redacted".into(),
        })
    }
    /// # Errors
    /// Rejects unavailable protected state, invalid scope or conflicting relationship changes.
    pub fn arm(&self) -> eyre::Result<()> {
        eyre::ensure!(
            self.runtime_status() != PhoneRuntimeState::Failed,
            "phone actor failed; restart broker before pairing"
        );
        let mut record = self
            .record
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("phone state unavailable"))?;
        eyre::ensure!(
            !self.pairing_live()?
                && record
                    .as_ref()
                    .is_none_or(|r| r.phone_certificate.is_none()),
            "phone already paired or pairing active; revoke before replacing"
        );
        let address = match &self.hub {
            PreviewHub::Resident => PhoneProfile::load(&AppHome::resolve()?)?.desktop.get(),
            #[cfg(test)]
            PreviewHub::Isolated(_) => Ipv4Addr::LOCALHOST,
        };
        // A revoked or abandoned identity may contain the previous profile's IP
        // certificate. Every explicit new attempt binds a fresh identity to the
        // current desktop address and invalidates older enrollment generations.
        let identity = identity::generate(address)?;
        self.store.save(&identity)?;
        *record = Some(identity);
        let until = Instant::now() + ENROLLMENT_WINDOW;
        *self
            .pair_until
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("phone state unavailable"))? = Some(until);
        *self
            .confirmation
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("pairing unavailable"))? =
            Some(pairing_confirmation::Attempt::new(
                until,
                wire::unix_ms()? + u64::try_from(ENROLLMENT_WINDOW.as_millis())?,
            ));
        Ok(())
    }
    /// # Errors
    /// Rejects unavailable protected state, invalid scope or conflicting relationship changes.
    pub fn revoke(&self) -> eyre::Result<()> {
        let mut record = self
            .record
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("phone state unavailable"))?;
        self.cancel_pairing_comparison();
        self.credential_hub().cancel_current();
        self.permission
            .lock()
            .map_err(|_redacted| eyre::eyre!("permission unavailable"))?
            .take();
        *self
            .pair_until
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("phone state unavailable"))? = None;
        if let Some(current) = record.as_ref() {
            let mut next = current.clone();
            next.phone_certificate = None;
            next.generation = uuid::Uuid::new_v4().to_string();
            self.store.save(&next)?;
            *record = Some(next);
        }
        Ok(())
    }
    pub(crate) fn pairing_attempt(
        &self,
        id: &str,
    ) -> eyre::Result<Arc<pairing_confirmation::Attempt>> {
        self.confirmation
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("pairing unavailable"))?
            .as_ref()
            .filter(|a| a.id == id)
            .cloned()
            .ok_or_else(|| eyre::eyre!("pairing attempt unavailable"))
    }
    pub(super) fn capture_pairing(
        &self,
    ) -> eyre::Result<(Record, Instant, Arc<pairing_confirmation::Attempt>)> {
        let record = self
            .record
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("pairing unavailable"))?;
        let identity = record
            .as_ref()
            .ok_or_else(|| eyre::eyre!("phone not enabled"))?;
        eyre::ensure!(identity.phone_certificate.is_none(), "phone already paired");
        let until = self
            .pair_until
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("pairing unavailable"))?
            .ok_or_else(|| eyre::eyre!("pairing unavailable"))?;
        let attempt = self
            .confirmation
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("pairing unavailable"))?
            .as_ref()
            .map(Arc::clone)
            .ok_or_else(|| eyre::eyre!("pairing unavailable"))?;
        eyre::ensure!(
            Instant::now() < until && !attempt.finished(),
            "pairing expired"
        );
        Ok((identity.clone(), until, attempt))
    }
    pub(super) fn pairing_is_current(
        &self,
        generation: &str,
        attempt: &Arc<pairing_confirmation::Attempt>,
    ) -> bool {
        let Ok(record) = self.record.lock() else {
            return false;
        };
        record
            .as_ref()
            .is_some_and(|r| r.generation == generation && r.phone_certificate.is_none())
            && self
                .confirmation
                .lock()
                .is_ok_and(|current| current.as_ref().is_some_and(|a| Arc::ptr_eq(a, attempt)))
            && !attempt.finished()
    }
    pub(super) fn finish_pairing_attempt(&self, attempt: &Arc<pairing_confirmation::Attempt>) {
        let Ok(_record) = self.record.lock() else {
            attempt.cancel();
            return;
        };
        attempt.cancel();
        if let Ok(mut current) = self.confirmation.lock()
            && current.as_ref().is_some_and(|a| Arc::ptr_eq(a, attempt))
        {
            *current = None;
            if let Ok(mut until) = self.pair_until.lock() {
                *until = None;
            }
        }
    }
    pub(super) fn cancel_pairing_comparison(&self) {
        if let Ok(mut active) = self.confirmation.lock()
            && let Some(attempt) = active.take()
        {
            attempt.cancel();
        }
    }
    /// # Errors
    /// Rejects invalid metadata, unavailable state, cancellation or expiry.
    pub fn authorize_once(&self, request: AcquireRequest, supply: bool) -> eyre::Result<()> {
        self.authorize_mode(request, supply, false)
    }
    /// # Errors
    /// Rejects non-fixture metadata or unavailable paired state.
    pub fn authorize_fixture(&self, request: AcquireRequest) -> eyre::Result<()> {
        synthetic_credential_fixture::validate(&request)?;
        self.authorize_mode(request, true, true)
    }
    fn authorize_mode(
        &self,
        request: AcquireRequest,
        supply: bool,
        synthetic_fixture: bool,
    ) -> eyre::Result<()> {
        let record = self
            .record
            .lock()
            .map_err(|_redacted| eyre::eyre!("relationship unavailable"))?;
        eyre::ensure!(
            record
                .as_ref()
                .is_some_and(|r| r.phone_certificate.is_some())
                && !self.pairing_live()?,
            "paired phone required"
        );
        let mut permission = CredentialPermission::new(request, supply)?;
        permission.synthetic_fixture = synthetic_fixture;
        permission.generation = record.as_ref().map(|r| r.generation.clone());
        let mut guard = self
            .permission
            .lock()
            .map_err(|_redacted| eyre::eyre!("permission unavailable"))?;
        if guard.as_ref().is_some_and(|p| !p.live()) {
            guard.take();
        }
        eyre::ensure!(
            guard.is_none(),
            "permission already pending; restart or wait for matching request"
        );
        *guard = Some(permission);
        Ok(())
    }
    /// # Errors
    /// Rejects invalid metadata, unavailable state, cancellation or expiry.
    pub fn claim_permission(
        self: &Arc<Self>,
        request: &AcquireRequest,
    ) -> eyre::Result<Option<Arc<CredentialGrant>>> {
        self.claim_mode(request, false)
    }
    pub(crate) fn claim_fixture_permission(
        self: &Arc<Self>,
        request: &AcquireRequest,
    ) -> eyre::Result<Arc<CredentialGrant>> {
        synthetic_credential_fixture::validate(request)?;
        self.claim_mode(request, true)?
            .ok_or_else(|| eyre::eyre!("fixture permission absent or expired"))
    }
    fn claim_mode(
        self: &Arc<Self>,
        request: &AcquireRequest,
        fixture: bool,
    ) -> eyre::Result<Option<Arc<CredentialGrant>>> {
        let record = self
            .record
            .lock()
            .map_err(|_redacted| eyre::eyre!("relationship unavailable"))?;
        let mut permission = self
            .permission
            .lock()
            .map_err(|_redacted| eyre::eyre!("permission unavailable"))?;
        if !permission.as_ref().is_some_and(|p| p.matches(request)) {
            return Ok(None);
        }
        eyre::ensure!(
            permission
                .as_ref()
                .is_some_and(|p| p.synthetic_fixture == fixture),
            "typed fixture permission required"
        );
        let consumed = permission
            .take()
            .ok_or_else(|| eyre::eyre!("permission unavailable"))?;
        let supply = consumed.supply();
        let synthetic_fixture = consumed.synthetic_fixture;
        let generation = consumed
            .generation
            .ok_or_else(|| eyre::eyre!("permission unbound"))?;
        eyre::ensure!(
            record
                .as_ref()
                .is_some_and(|r| r.generation == generation && r.phone_certificate.is_some()),
            "permission revoked"
        );
        Ok(Some(Arc::new(CredentialGrant {
            controller: Arc::downgrade(self),
            generation,
            request: request.clone(),
            supply,
            synthetic_fixture,
        })))
    }
    pub(crate) fn with_generation<T>(
        &self,
        generation: &str,
        action: impl FnOnce() -> eyre::Result<T>,
    ) -> eyre::Result<T> {
        let record = self
            .record
            .lock()
            .map_err(|_redacted| eyre::eyre!("relationship unavailable"))?;
        eyre::ensure!(
            record
                .as_ref()
                .is_some_and(|r| r.generation == generation && r.phone_certificate.is_some()),
            "permission revoked"
        );
        action()
    }
    pub(super) fn snapshot(&self) -> Option<SharedPreviewSnapshot> {
        self.credential_hub()
            .snapshot()
            .or_else(|| self.hub().snapshot())
    }
    fn runtime_status(&self) -> PhoneRuntimeState {
        match self.runtime.load(Ordering::Acquire) {
            0 => PhoneRuntimeState::Stopped,
            1 => PhoneRuntimeState::Running,
            2 => PhoneRuntimeState::Recovering,
            _ => PhoneRuntimeState::Failed,
        }
    }
    pub(super) fn recovering(&self) {
        if self.runtime.swap(2, Ordering::AcqRel) != 2 {
            tracing::warn!(
                phase = "phone-listener-unavailable",
                "No-vault phone listener unavailable; bounded retry pending"
            );
        }
    }
    pub(crate) fn credential_hub(&self) -> &SharedPreviewHub {
        match &self.credential_hub {
            PreviewHub::Resident => credential_approvals::hub(),
            #[cfg(test)]
            PreviewHub::Isolated(hub) => hub,
        }
    }
    fn hub(&self) -> &SharedPreviewHub {
        match &self.hub {
            PreviewHub::Resident => SharedPreviewHub::resident(),
            #[cfg(test)]
            PreviewHub::Isolated(hub) => hub,
        }
    }
    pub(super) fn resolve(
        &self,
        generation: &str,
        certificate: &[u8],
        id: &str,
        decision: ApprovalPreviewDisposition,
    ) -> eyre::Result<bool> {
        let record = self
            .record
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("phone state unavailable"))?;
        eyre::ensure!(
            record.as_ref().is_some_and(|r| r.generation == generation
                && r.phone_certificate.as_deref() == Some(certificate)),
            "phone revoked or replaced"
        );
        if decision == ApprovalPreviewDisposition::ApprovedTest
            && self
                .credential_hub()
                .snapshot()
                .is_some_and(|s| s.request_id == id)
        {
            return Ok(self.credential_hub().request_provider(id));
        }
        Ok(self.credential_hub().decide(id, decision).is_ok()
            || self.hub().decide(id, decision).is_ok())
    }
    pub(super) fn supply(
        &self,
        generation: &str,
        certificate: &[u8],
        id: &str,
        bytes: zeroize::Zeroizing<Vec<u8>>,
    ) -> eyre::Result<bool> {
        let record = self
            .record
            .lock()
            .map_err(|_redacted| eyre::eyre!("phone state unavailable"))?;
        eyre::ensure!(
            record.as_ref().is_some_and(|r| r.generation == generation
                && r.phone_certificate.as_deref() == Some(certificate)),
            "phone revoked or replaced"
        );
        Ok(self.credential_hub().supply(id, bytes))
    }
    pub(super) fn pairing_live(&self) -> eyre::Result<bool> {
        Ok(self
            .pair_until
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("phone state unavailable"))?
            .is_some_and(|until| Instant::now() < until))
    }
    pub(super) fn record(&self) -> eyre::Result<Option<Record>> {
        Ok(self
            .record
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("phone state unavailable"))?
            .clone())
    }
    pub(super) fn authorized(&self, generation: &str, certificate: &[u8]) -> eyre::Result<bool> {
        Ok(self.record()?.is_some_and(|r| {
            r.generation == generation && r.phone_certificate.as_deref() == Some(certificate)
        }))
    }
    pub(super) fn enroll(&self, generation: &str, certificate: &[u8]) -> eyre::Result<()> {
        let mut guard = self
            .record
            .lock()
            .map_err(|_redacted_error| eyre::eyre!("phone state unavailable"))?;
        eyre::ensure!(
            self.pairing_live()?
                && guard
                    .as_ref()
                    .is_some_and(|r| r.generation == generation && r.phone_certificate.is_none()),
            "pairing cancelled"
        );
        let mut record = guard
            .as_ref()
            .ok_or_else(|| eyre::eyre!("phone state unavailable"))?
            .clone();
        record.phone_certificate = Some(certificate.to_vec());
        let _verified = identity::acceptor(&record)?;
        self.store.save(&record)?;
        *guard = Some(record);
        Ok(())
    }
}
