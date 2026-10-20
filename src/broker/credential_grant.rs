use crate::protocol::AcquireRequest;
use crate::protocol::credential_context::CredentialContext;
use std::sync::Arc;
use std::sync::Weak;
#[derive(Debug)]
pub struct CredentialGrant {
    pub(crate) controller: Weak<super::phone::Controller>,
    pub(crate) generation: String,
    pub(crate) request: AcquireRequest,
    pub(crate) supply: bool,
    pub(crate) synthetic_fixture: bool,
}
impl CredentialGrant {
    pub(crate) fn with_valid<T>(
        &self,
        action: impl FnOnce() -> eyre::Result<T>,
    ) -> eyre::Result<T> {
        self.controller
            .upgrade()
            .ok_or_else(|| eyre::eyre!("relationship unavailable"))?
            .with_generation(&self.generation, action)
    }
    pub(crate) fn begin(
        self: &Arc<Self>,
        request: AcquireRequest,
        pid: u32,
        cancel: Arc<std::sync::atomic::AtomicBool>,
    ) -> eyre::Result<Arc<super::shared_preview::SharedPreview>> {
        eyre::ensure!(request == self.request, "exact permission changed");
        self.with_valid(|| {
            self.controller
                .upgrade()
                .ok_or_else(|| eyre::eyre!("relationship unavailable"))?
                .credential_hub()
                .begin_bound(
                    cancel,
                    CredentialContext {
                        request,
                        supply: self.supply,
                        synthetic_fixture: self.synthetic_fixture,
                        connected_process_id: pid,
                        provider_state: "idle".into(),
                    },
                    Arc::clone(self),
                )
        })
    }
}
