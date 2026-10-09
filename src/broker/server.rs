use super::cancel_guard::CancelGuard;
use super::engine::Engine;
use crate::protocol::Hello;
use crate::protocol::Request;
use crate::protocol::Response;
use crate::protocol::{self};
use std::os::windows::io::AsRawHandle;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use teamy_cancellation::CancellationToken;
use tokio::io::AsyncReadExt;
use tokio::net::windows::named_pipe::NamedPipeServer;
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub async fn serve(engine: Arc<Engine>, cancellation: CancellationToken) -> eyre::Result<()> {
    let endpoint = crate::platform::identity::endpoint()?;
    serve_at(engine, cancellation, &endpoint, true).await
}
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub async fn serve_at(
    engine: Arc<Engine>,
    cancellation: CancellationToken,
    endpoint: &str,
    reconcile: bool,
) -> eyre::Result<()> {
    let mut listener = crate::platform::pipe_security::create(endpoint, true)?;
    // First pipe instance remains held throughout startup: racing servers cannot reconcile live leases.
    if reconcile {
        super::persistence::reconcile()?;
    }
    let instance = uuid::Uuid::new_v4().to_string();
    let mut clients = tokio::task::JoinSet::new();
    let slots = Arc::new(tokio::sync::Semaphore::new(32));
    let mut maintenance = tokio::time::interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            result=listener.connect()=>{
                result?;
                let next=crate::platform::pipe_security::create(endpoint,false)?;
                let connected=std::mem::replace(&mut listener,next);
                if let Ok(permit)=Arc::clone(&slots).try_acquire_owned(){
                    let engine=Arc::clone(&engine);let instance=instance.clone();
                    clients.spawn(async move {let _permit=permit;let _=session(connected,engine,instance).await;});
                }
            }
            _=maintenance.tick()=>{
                if cancellation.is_cancelled() || engine.stop_ready.load(Ordering::SeqCst){break}
                engine.expire().await?;
            }
            _=clients.join_next(),if !clients.is_empty()=>{}
        }
    }
    engine.stopping.store(true, Ordering::SeqCst);
    clients.abort_all();
    while clients.join_next().await.is_some() {}
    engine.shutdown().await?;
    drop(listener);
    Ok(())
}
async fn session(
    mut pipe: NamedPipeServer,
    engine: Arc<Engine>,
    instance_id: String,
) -> eyre::Result<()> {
    let hello = Hello {
        version: protocol::VERSION,
        capabilities: protocol::CAPABILITIES
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        instance_id,
    };
    protocol::wire::write(&mut pipe, &hello).await?;
    let peer: Hello =
        tokio::time::timeout(Duration::from_secs(5), protocol::wire::read(&mut pipe)).await??;
    eyre::ensure!(peer.compatible(), "incompatible SDK");
    let request: Request =
        tokio::time::timeout(Duration::from_secs(5), protocol::wire::read(&mut pipe)).await??;
    request.validate()?;
    let is_stop = request.operation == "stop";
    let response = match request.operation.as_str() {
        "acquire" => {
            let pid = peer_pid(&pipe)?;
            let cancel = Arc::new(AtomicBool::new(false));
            let _guard = CancelGuard(Arc::clone(&cancel));
            let acquire = engine.acquire(
                request
                    .acquire
                    .ok_or_else(|| eyre::eyre!("missing acquire"))?,
                pid,
                cancel,
            );
            // Client EOF, invalid extra input, timeout or server shutdown drops the adapter future and cancels UI.
            let grant = tokio::select! {result=tokio::time::timeout(Duration::from_secs(185),acquire)=>result.map_err(|_redacted_error|eyre::eyre!("request timeout"))?, _=pipe.read_u8()=>return Ok(())};
            match grant {
                Ok(grant) => {
                    eyre::ensure!(
                        protocol::wire::unix_ms()? < grant.expires
                            && std::time::Instant::now() < grant.deadline
                            && !engine.stopping.load(Ordering::SeqCst),
                        "expired"
                    );
                    protocol::wire::write(
                        &mut pipe,
                        &Response {
                            code: "granted".into(),
                            lease_id: Some(grant.id),
                            expires_unix_ms: Some(grant.expires),
                            remaining_millis: Some(u64::try_from(
                                grant
                                    .deadline
                                    .saturating_duration_since(std::time::Instant::now())
                                    .as_millis(),
                            )?),
                            status: None,
                        },
                    )
                    .await?;
                    protocol::wire::write_secret(&mut pipe, &grant.bytes).await?;
                    return Ok(());
                }
                Err(_) => Response::code("denied"),
            }
        }
        "validate" => match engine
            .validate_lease(
                request
                    .lease_id
                    .as_deref()
                    .ok_or_else(|| eyre::eyre!("missing lease ID"))?,
            )
            .await
        {
            Ok(reply) => reply,
            Err(_redacted_error) => Response::code("denied"),
        },
        "status" => Response {
            code: "ok".into(),
            status: Some(engine.status().await?),
            ..Response::default()
        },
        "purge" => {
            if engine.purge(request.lease_id.as_deref()).await.is_err() {
                protocol::wire::write(&mut pipe, &Response::code("cleanup_failed")).await?;
                return Ok(());
            }
            Response::code("ok")
        }
        "stop" => match engine.shutdown().await {
            Ok(()) => Response::code("ok"),
            Err(_redacted_error) => Response::code("cleanup_failed"),
        },
        _ => Response::code("unsupported"),
    };
    tokio::time::timeout(
        Duration::from_secs(5),
        protocol::wire::write(&mut pipe, &response),
    )
    .await??;
    if is_stop {
        engine.stop_ready.store(true, Ordering::SeqCst);
    }
    Ok(())
}

fn peer_pid(pipe: &NamedPipeServer) -> eyre::Result<u32> {
    let mut pid = 0;
    // SAFETY: pipe owns a connected live named-pipe handle; PID output is writable.
    let result = unsafe {
        windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId(
            pipe.as_raw_handle().cast(),
            &raw mut pid,
        )
    };
    eyre::ensure!(result != 0, "peer identity unavailable");
    Ok(pid)
}
