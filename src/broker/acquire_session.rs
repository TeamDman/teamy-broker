use crate::protocol::Hello;
use crate::protocol::Request;
use crate::protocol::Response;
use crate::protocol::{self};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::windows::named_pipe::NamedPipeServer;
pub(super) async fn execute(
    pipe: &mut NamedPipeServer,
    engine: &Arc<super::engine::Engine>,
    peer: &Hello,
    request: Request,
) -> eyre::Result<()> {
    let fixture = request.operation == "acquire-fixture";
    if fixture {
        eyre::ensure!(
            peer.capabilities
                .iter()
                .any(|c| c == "isolated-credential-fixture-v1"),
            "fixture capability required"
        );
    }
    let pid = super::server::peer_pid(pipe)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let _guard = super::cancel_guard::CancelGuard(Arc::clone(&cancel));
    let acquire = async {
        let metadata = request
            .acquire
            .ok_or_else(|| eyre::eyre!("missing acquire"))?;
        if fixture {
            engine.acquire_fixture(metadata, pid, cancel).await
        } else {
            engine.acquire(metadata, pid, cancel).await
        }
    };
    // EOF/extra input, request timeout and shutdown drop the future and cancel this request.
    let grant = tokio::select! {result=tokio::time::timeout(Duration::from_secs(185),acquire)=>result.unwrap_or_else(|_redacted|Err(protocol::FailureKind::RequestTimedOut.into())),_=pipe.read_u8()=>return Ok(())};
    match grant {
        Ok(grant) => super::server::send_grant(pipe, engine, grant).await,
        Err(error) => {
            protocol::wire::write(
                pipe,
                &Response::failure(
                    error
                        .downcast_ref::<protocol::FailureKind>()
                        .copied()
                        .unwrap_or(protocol::FailureKind::RequestFailed),
                ),
            )
            .await
        }
    }
}
