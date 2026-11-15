use super::engine::Engine;
use crate::platform::identity;
use crate::platform::pipe_security;
use crate::protocol::Hello;
use crate::protocol::Request;
use crate::protocol::Response;
use crate::protocol::{self};
use std::mem;
use std::os::windows::io::AsRawHandle;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use teamy_cancellation::CancellationToken;
use tokio::net::windows::named_pipe::NamedPipeServer;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time;
use windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId;
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub async fn serve(engine: Arc<Engine>, cancellation: CancellationToken) -> eyre::Result<()> {
    tracing::info!(phase = "identity-query", "Broker startup");
    let endpoint = identity::endpoint()?;
    tracing::info!(phase = "identity-ready", "Broker startup");
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
    serve_with_listener_factory(
        engine,
        cancellation,
        endpoint,
        reconcile,
        pipe_security::create,
    )
    .await
}

async fn serve_with_listener_factory(
    engine: Arc<Engine>,
    cancellation: CancellationToken,
    endpoint: &str,
    reconcile: bool,
    create_listener: impl Fn(&str, bool) -> eyre::Result<NamedPipeServer> + Send + Sync,
) -> eyre::Result<()> {
    let mut listener = create_listener(endpoint, true)?;
    tracing::info!(phase = "pipe-created", "Broker startup");
    let mut phone = JoinSet::new();
    let mut clients = JoinSet::new();
    let mut maintenance = JoinSet::new();
    let result = async {
        // Keep the first pipe instance throughout startup and all final cleanup.
        if reconcile {
            tracing::info!(phase = "reconcile", "Broker startup");
            super::persistence::reconcile()?;
        }
        if reconcile && endpoint == identity::endpoint()? {
            let controller = super::phone::Controller::resident()?;
            let cancellation = cancellation.clone();
            let engine = Arc::clone(&engine);
            phone.spawn(async move {
                if super::phone::run(controller, cancellation, engine).await.is_err() {
                    tracing::error!(
                        phase = "phone-runtime-failed",
                        "No-vault phone actor stopped; inspect phone-status"
                    );
                }
            });
        }
        tracing::info!(phase = "ready", "Broker ready");
        let instance = uuid::Uuid::new_v4().to_string();
        let slots = Arc::new(Semaphore::new(32));
        let mut maintenance_tick = time::interval(Duration::from_millis(100));
        loop {
            tokio::select! {
                connected=listener.connect()=>{
                    connected?;
                    let next=create_listener(endpoint,false)?;
                    let connected=mem::replace(&mut listener,next);
                    if let Ok(permit)=Arc::clone(&slots).try_acquire_owned(){
                        let engine=Arc::clone(&engine);let instance=instance.clone();
                        clients.spawn(async move {let _permit=permit;let _=session(connected,engine,instance).await;});
                    }
                }
                _=maintenance_tick.tick()=>{
                    if cancellation.is_cancelled() || engine.stop_ready.load(Ordering::SeqCst){break}
                    if maintenance.is_empty() {
                        let engine = Arc::clone(&engine);
                        maintenance.spawn(async move { engine.expire().await });
                    }
                }
                completed=maintenance.join_next(),if !maintenance.is_empty()=>{
                    if let Some(completed) = completed {
                        completed.map_err(|_panic| eyre::eyre!("broker maintenance worker failed"))??;
                    }
                }
                _=clients.join_next(),if !clients.is_empty()=>{}
            }
        }
        Ok::<(), eyre::Report>(())
    }.await;
    engine.stopping.store(true, Ordering::SeqCst);
    phone.abort_all();
    clients.abort_all();
    maintenance.abort_all();
    while phone.join_next().await.is_some() {}
    while clients.join_next().await.is_some() {}
    while maintenance.join_next().await.is_some() {}
    // Aborted async maintenance does not abandon its owned native cleanup workers.
    // Cleanup failure takes priority over an accept/maintenance error for recovery.
    let cleaned = engine.shutdown().await;
    drop(listener);
    cleaned?;
    result
}
async fn session(
    mut pipe: NamedPipeServer,
    engine: Arc<Engine>,
    instance_id: String,
) -> eyre::Result<()> {
    let hello = server_hello(instance_id);
    protocol::wire::write(&mut pipe, &hello).await?;
    let peer: Hello =
        time::timeout(Duration::from_secs(5), protocol::wire::read(&mut pipe)).await??;
    eyre::ensure!(peer.compatible(), "incompatible SDK");
    let request: Request =
        time::timeout(Duration::from_secs(5), protocol::wire::read(&mut pipe)).await??;
    request.validate()?;
    let is_stop = matches!(request.operation.as_str(), "stop" | "stop-force");
    let _control_work = if matches!(
        request.operation.as_str(),
        "purge" | "validate" | "phone-pair" | "phone-revoke"
    ) {
        Some(engine.control_work()?)
    } else {
        None
    };
    let response = match request.operation.as_str() {
        operation if is_phone_control(operation) => {
            return super::phone_control_session::execute(&mut pipe, &peer, &request).await;
        }
        "preview-state" | "preview-approve" | "preview-deny" | "preview-cancel" => {
            return super::shared_preview_session::execute(&mut pipe, &peer, &request).await;
        }
        "approval-preview" => {
            return super::approval_preview_session::execute(&mut pipe, &engine, &peer).await;
        }
        "acquire" | "acquire-fixture" => {
            return super::acquire_session::execute(&mut pipe, &engine, &peer, request).await;
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
        "stop" | "stop-force" => match engine.stop(request.operation == "stop-force").await {
            Ok(()) => Response::code("ok"),
            Err(error)
                if error.downcast_ref::<protocol::FailureKind>()
                    == Some(&protocol::FailureKind::ShutdownBlocked) =>
            {
                Response::failure(protocol::FailureKind::ShutdownBlocked)
            }
            Err(_redacted_error) => Response::code("cleanup_failed"),
        },
        _ => Response::code("unsupported"),
    };
    let written = time::timeout(
        Duration::from_secs(5),
        protocol::wire::write(&mut pipe, &response),
    )
    .await;
    if is_stop && response.code == "ok" {
        engine.stop_ready.store(true, Ordering::SeqCst);
    }
    written??;
    Ok(())
}

pub(super) fn peer_pid(pipe: &NamedPipeServer) -> eyre::Result<u32> {
    let mut pid = 0;
    // SAFETY: pipe owns a connected live named-pipe handle; PID output is writable.
    let result = unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle().cast(), &raw mut pid) };
    eyre::ensure!(result != 0, "peer identity unavailable");
    Ok(pid)
}

pub(super) async fn send_grant(
    pipe: &mut NamedPipeServer,
    engine: &Engine,
    grant: super::grant::Grant,
) -> eyre::Result<()> {
    eyre::ensure!(
        protocol::wire::unix_ms()? < grant.expires
            && Instant::now() < grant.deadline
            && !engine.stopping.load(Ordering::SeqCst),
        "expired"
    );
    protocol::wire::write(
        pipe,
        &Response {
            code: "granted".into(),
            lease_id: Some(grant.id),
            expires_unix_ms: Some(grant.expires),
            remaining_millis: Some(u64::try_from(
                grant
                    .deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis(),
            )?),
            status: None,
            failure: None,
        },
    )
    .await?;
    protocol::wire::write_secret(pipe, &grant.bytes).await?;
    Ok(())
}

fn server_hello(instance_id: String) -> Hello {
    Hello {
        version: protocol::VERSION,
        capabilities: protocol::CAPABILITIES
            .iter()
            .copied()
            .chain([
                "shutdown-state-v1",
                "stop-force",
                protocol::APPROVAL_PREVIEW_CAPABILITY,
                protocol::SHARED_PREVIEW_CAPABILITY,
                "persistent-no-vault-phone-v1",
                "isolated-credential-fixture-v1",
            ])
            .map(str::to_string)
            .collect(),
        instance_id,
    }
}

fn is_phone_control(operation: &str) -> bool {
    matches!(
        operation,
        "phone-pair"
            | "phone-revoke"
            | "phone-list"
            | "phone-status"
            | "phone-pair-inspect"
            | "phone-pair-confirm"
            | "phone-pair-cancel"
            | "phone-authorize-desktop"
            | "phone-authorize-supply"
            | "phone-authorize-fixture"
    )
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
