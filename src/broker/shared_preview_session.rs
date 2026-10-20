use crate::protocol::ApprovalPreviewDisposition;
use crate::protocol::Hello;
use crate::protocol::Request;
use crate::protocol::Response;
use crate::protocol::SharedPreviewState;
use crate::protocol::{self};
use tokio::net::windows::named_pipe::NamedPipeServer;
pub(super) async fn execute(
    pipe: &mut NamedPipeServer,
    peer: &Hello,
    request: &Request,
) -> eyre::Result<()> {
    eyre::ensure!(
        peer.capabilities
            .iter()
            .any(|cap| cap == protocol::SHARED_PREVIEW_CAPABILITY),
        "shared preview capability not negotiated"
    );
    let hub = super::shared_preview::SharedPreviewHub::resident();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        if request.operation == "preview-state" {
            protocol::wire::write(pipe, &Response::code("preview-state")).await?;
            protocol::wire::write(
                pipe,
                &SharedPreviewState {
                    pending: hub.snapshot(),
                },
            )
            .await
        } else {
            let disposition = match request.operation.as_str() {
                "preview-approve" => ApprovalPreviewDisposition::ApprovedTest,
                "preview-deny" => ApprovalPreviewDisposition::DeniedTest,
                "preview-cancel" => ApprovalPreviewDisposition::Cancelled,
                _ => eyre::bail!("unsupported preview decision"),
            };
            let accepted = hub
                .decide(
                    request
                        .lease_id
                        .as_deref()
                        .ok_or_else(|| eyre::eyre!("missing preview ID"))?,
                    disposition,
                )
                .is_ok();
            protocol::wire::write(
                pipe,
                &Response::code(if accepted {
                    "preview-accepted"
                } else {
                    "preview-stale"
                }),
            )
            .await
        }
    })
    .await??;
    Ok(())
}
