use super::Client;
use crate::protocol::APPROVAL_PREVIEW_CAPABILITY;
use crate::protocol::ApprovalPreviewResult;
use crate::protocol::FailureKind;
use crate::protocol::Request;
use crate::protocol::Response;
use crate::protocol::wire;
use std::time::Duration;
use tokio::time::timeout;
impl Client {
    /// Explicit no-vault resident test; never autostarts or falls back to acquisition.
    /// # Errors
    /// Fails before sending a request if the optional preview capability is absent.
    pub async fn approval_preview(&self) -> eyre::Result<ApprovalPreviewResult> {
        timeout(Duration::from_secs(70), async {
            let mut pipe = self
                .connect(false, Some(APPROVAL_PREVIEW_CAPABILITY))
                .await?;
            wire::write(&mut pipe, &Request::control("approval-preview", None)).await?;
            let reply: Response = wire::read(&mut pipe).await?;
            if reply.code != "preview-complete" {
                return Err(reply.failure.unwrap_or(FailureKind::RequestFailed).into());
            }
            eyre::ensure!(
                reply.failure.is_none()
                    && reply.lease_id.is_none()
                    && reply.expires_unix_ms.is_none()
                    && reply.remaining_millis.is_none()
                    && reply.status.is_none(),
                "invalid preview response"
            );
            wire::read(&mut pipe).await
        })
        .await
        .map_err(|_timeout| eyre::Report::new(FailureKind::RequestTimedOut))?
    }
}
