use super::Client;
use crate::protocol::ApprovalPreviewDisposition;
use crate::protocol::Request;
use crate::protocol::Response;
use crate::protocol::SharedPreviewState;
use crate::protocol::{self};
impl Client {
    /// No autostart. Snapshot reconciliation clears requests resolved elsewhere or before reconnect.
    /// # Errors
    /// Rejects an unavailable/legacy resident or malformed response.
    pub async fn shared_preview_state(&self) -> eyre::Result<SharedPreviewState> {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let mut pipe = self
                .connect(false, Some(protocol::SHARED_PREVIEW_CAPABILITY))
                .await?;
            protocol::wire::write(&mut pipe, &Request::control("preview-state", None)).await?;
            let response: Response = protocol::wire::read(&mut pipe).await?;
            eyre::ensure!(
                response.code == "preview-state"
                    && response.lease_id.is_none()
                    && response.failure.is_none(),
                "invalid preview state"
            );
            protocol::wire::read(&mut pipe).await
        })
        .await?
    }
    /// Resolve exactly one no-vault request. An accepted decision never authorizes acquisition.
    /// # Errors
    /// Rejects stale IDs, invalid dispositions, timeout, or a legacy resident.
    pub async fn shared_preview_decide(
        &self,
        id: &str,
        decision: ApprovalPreviewDisposition,
    ) -> eyre::Result<bool> {
        let operation = match decision {
            ApprovalPreviewDisposition::ApprovedTest => "preview-approve",
            ApprovalPreviewDisposition::DeniedTest => "preview-deny",
            ApprovalPreviewDisposition::Cancelled => "preview-cancel",
            _ => eyre::bail!("invalid preview decision"),
        };
        let request = Request::control(operation, Some(id.into()));
        request.validate()?;
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let mut pipe = self
                .connect(false, Some(protocol::SHARED_PREVIEW_CAPABILITY))
                .await?;
            protocol::wire::write(&mut pipe, &request).await?;
            let response: Response = protocol::wire::read(&mut pipe).await?;
            eyre::ensure!(
                matches!(response.code.as_str(), "preview-accepted" | "preview-stale")
                    && response.lease_id.is_none()
                    && response.failure.is_none(),
                "invalid preview decision response"
            );
            Ok(response.code == "preview-accepted")
        })
        .await?
    }
}
