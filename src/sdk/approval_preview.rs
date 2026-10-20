use super::Client;
use crate::protocol::ApprovalPreviewResult;
use crate::protocol::Request;
use crate::protocol::Response;
impl Client {
    /// Explicit no-vault resident test; never autostarts or falls back to acquisition.
    /// # Errors
    /// Fails before sending a request if the optional preview capability is absent.
    pub async fn approval_preview(&self) -> eyre::Result<ApprovalPreviewResult> {
        tokio::time::timeout(std::time::Duration::from_secs(70), async {
            let mut pipe = self
                .connect(false, Some(crate::protocol::APPROVAL_PREVIEW_CAPABILITY))
                .await?;
            crate::protocol::wire::write(&mut pipe, &Request::control("approval-preview", None))
                .await?;
            let reply: Response = crate::protocol::wire::read(&mut pipe).await?;
            if reply.code != "preview-complete" {
                return Err(reply
                    .failure
                    .unwrap_or(crate::protocol::FailureKind::RequestFailed)
                    .into());
            }
            eyre::ensure!(
                reply.failure.is_none()
                    && reply.lease_id.is_none()
                    && reply.expires_unix_ms.is_none()
                    && reply.remaining_millis.is_none()
                    && reply.status.is_none(),
                "invalid preview response"
            );
            crate::protocol::wire::read(&mut pipe).await
        })
        .await
        .map_err(|_timeout| eyre::Report::new(crate::protocol::FailureKind::RequestTimedOut))?
    }
}
