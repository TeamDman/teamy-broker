use super::Client;
use crate::protocol::PhoneStatus;
use crate::protocol::Request;
use crate::protocol::Response;
use crate::protocol::pairing_comparison::PairingCode;
use crate::protocol::pairing_comparison::PairingConfirmationResult;
use crate::protocol::pairing_comparison::PairingInspection;
use crate::protocol::wire;
use std::time::Duration;
use tokio::net::windows::named_pipe::NamedPipeClient;
use tokio::time::timeout;
impl Client {
    /// # Errors
    /// Rejects invalid metadata, unavailable state, cancellation or expiry.
    pub async fn phone_authorize_once(&self, request: Request) -> eyre::Result<PhoneStatus> {
        request.validate()?;
        eyre::ensure!(
            matches!(
                request.operation.as_str(),
                "phone-authorize-desktop" | "phone-authorize-supply" | "phone-authorize-fixture"
            ),
            "permission operation rejected"
        );
        timeout(self.request_timeout.min(Duration::from_secs(5)), async {
            let mut pipe = self
                .connect(false, Some("persistent-no-vault-phone-v1"))
                .await?;
            wire::write(&mut pipe, &request).await?;
            let reply: Response = wire::read(&mut pipe).await?;
            eyre::ensure!(
                reply.code == "phone-status",
                "per-action permission rejected"
            );
            wire::read(&mut pipe).await
        })
        .await
        .map_err(|_redacted_error| eyre::eyre!("phone exchange deadline"))?
    }
    /// Current-user secured pipe; no autostart, no certificate/key export.
    /// # Errors
    /// Rejects unavailable/legacy residents or invalid relationship controls.
    pub async fn phone_control(&self, operation: &str) -> eyre::Result<PhoneStatus> {
        eyre::ensure!(
            matches!(
                operation,
                "phone-pair" | "phone-revoke" | "phone-list" | "phone-status"
            ),
            "phone control rejected"
        );
        let request = Request::control(operation, None);
        request.validate()?;
        timeout(self.request_timeout.min(Duration::from_secs(5)), async {
            let mut pipe = self
                .connect(false, Some("persistent-no-vault-phone-v1"))
                .await?;
            wire::write(&mut pipe, &request).await?;
            let reply: Response = wire::read(&mut pipe).await?;
            eyre::ensure!(
                reply.code == "phone-status",
                "phone control rejected; inspect status and revoke before replacing"
            );
            wire::read(&mut pipe).await
        })
        .await
        .map_err(|_redacted_error| eyre::eyre!("phone exchange deadline"))?
    }
}

impl Client {
    async fn pairing_pipe(&self, operation: &str, id: &str) -> eyre::Result<NamedPipeClient> {
        let request = Request::control(operation, Some(id.into()));
        request.validate()?;
        let mut pipe = self
            .connect(false, Some("persistent-no-vault-phone-v1"))
            .await?;
        wire::write(&mut pipe, &request).await?;
        Ok(pipe)
    }
    /// # Errors
    /// Requires exact live attempt; deliberately reveals only its ephemeral comparison.
    pub async fn pairing_inspect(&self, id: &str) -> eyre::Result<PairingInspection> {
        timeout(self.request_timeout.min(Duration::from_secs(5)), async {
            let mut p = self.pairing_pipe("phone-pair-inspect", id).await?;
            let r: Response = wire::read(&mut p).await?;
            eyre::ensure!(r.code == "pairing-comparison", "comparison unavailable");
            wire::read(&mut p).await
        })
        .await
        .map_err(|_redacted_error| eyre::eyre!("pairing exchange deadline"))?
    }
    /// # Errors
    /// Requires exact live attempt and exact peer comparison; no latest/wildcard approval.
    pub async fn pairing_confirm(
        &self,
        id: &str,
        code: &PairingCode,
    ) -> eyre::Result<PairingConfirmationResult> {
        timeout(self.request_timeout.min(Duration::from_secs(5)), async {
            let mut p = self.pairing_pipe("phone-pair-confirm", id).await?;
            wire::write(&mut p, code).await?;
            let r: Response = wire::read(&mut p).await?;
            eyre::ensure!(r.code == "pairing-confirmation", "confirmation unavailable");
            wire::read(&mut p).await
        })
        .await
        .map_err(|_redacted_error| eyre::eyre!("pairing exchange deadline"))?
    }
    /// # Errors
    /// Cancels only the exact live comparison, without revoking stored trust.
    pub async fn pairing_cancel(&self, id: &str) -> eyre::Result<PairingConfirmationResult> {
        timeout(self.request_timeout.min(Duration::from_secs(5)), async {
            let mut p = self.pairing_pipe("phone-pair-cancel", id).await?;
            let r: Response = wire::read(&mut p).await?;
            eyre::ensure!(r.code == "pairing-confirmation", "cancellation unavailable");
            wire::read(&mut p).await
        })
        .await
        .map_err(|_redacted_error| eyre::eyre!("pairing exchange deadline"))?
    }
}
