use super::cancel_guard::CancelGuard;
use super::engine::Engine;
use crate::protocol::Hello;
use crate::protocol::Response;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::windows::named_pipe::NamedPipeServer;
pub(super) async fn execute(
    pipe: &mut NamedPipeServer,
    engine: &Engine,
    peer: &Hello,
) -> eyre::Result<()> {
    eyre::ensure!(
        peer.capabilities
            .iter()
            .any(|cap| cap == crate::protocol::APPROVAL_PREVIEW_CAPABILITY),
        "preview capability not negotiated"
    );
    let pid = super::server::peer_pid(pipe)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let _guard = CancelGuard(Arc::clone(&cancel));
    let preview = engine.approval_preview(pid, cancel);
    let result = tokio::select! {result=preview=>result,_=pipe.read_u8()=>return Ok(())};
    tokio::time::timeout(Duration::from_secs(5), async {
        match result {
            Ok(result) => {
                crate::protocol::wire::write(pipe, &Response::code("preview-complete")).await?;
                crate::protocol::wire::write(pipe, &result).await
            }
            Err(error) => {
                crate::protocol::wire::write(
                    pipe,
                    &Response::failure(
                        error
                            .downcast_ref::<crate::protocol::FailureKind>()
                            .copied()
                            .unwrap_or(crate::protocol::FailureKind::ApprovalUnavailable),
                    ),
                )
                .await
            }
        }
    })
    .await??;
    Ok(())
}
