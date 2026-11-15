use super::cancel_guard::CancelGuard;
use super::engine::Engine;
use crate::protocol::APPROVAL_PREVIEW_CAPABILITY;
use crate::protocol::FailureKind;
use crate::protocol::Hello;
use crate::protocol::Response;
use crate::protocol::wire;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::windows::named_pipe::NamedPipeServer;
use tokio::time;
pub(super) async fn execute(
    pipe: &mut NamedPipeServer,
    engine: &Engine,
    peer: &Hello,
) -> eyre::Result<()> {
    eyre::ensure!(
        peer.capabilities
            .iter()
            .any(|cap| cap == APPROVAL_PREVIEW_CAPABILITY),
        "preview capability not negotiated"
    );
    let pid = super::server::peer_pid(pipe)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let _guard = CancelGuard(Arc::clone(&cancel));
    let preview = engine.approval_preview(pid, cancel);
    let result = tokio::select! {result=preview=>result,_=pipe.read_u8()=>return Ok(())};
    time::timeout(Duration::from_secs(5), async {
        match result {
            Ok(result) => {
                wire::write(pipe, &Response::code("preview-complete")).await?;
                wire::write(pipe, &result).await
            }
            Err(error) => {
                wire::write(
                    pipe,
                    &Response::failure(
                        error
                            .downcast_ref::<FailureKind>()
                            .copied()
                            .unwrap_or(FailureKind::ApprovalUnavailable),
                    ),
                )
                .await
            }
        }
    })
    .await??;
    Ok(())
}
