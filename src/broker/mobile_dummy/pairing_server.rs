use super::java_pairing_worker::JavaPairingWorker;
use super::paired_socket::PairedSocket;
use super::paired_socket::read_frame;
use super::paired_socket::write_frame;
use super::pairing_prompt_state::PairingPromptState;
use super::preview_activation::PreviewActivation;
use super::preview_phase::PreviewPhase;
use super::preview_phase::PreviewTrace;
use super::preview_phase::emit;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use teamy_broker_handoff::pairing_binding::PairingBinding;
use teamy_broker_handoff::session::Session;
use tokio::net::TcpListener;
/// Legacy dummy-acquisition example gate. Production serve never registers a phone listener.
pub const PAIRING_ACTIVATION_APPROVED: bool = false;
/// # Errors
/// Legacy one-attempt dummy scope still requires its separate static activation gate.
pub async fn accept(
    listener: TcpListener,
    java: &Path,
    artifacts: &Path,
    binding: &PairingBinding,
    session: &Session,
    until: tokio::time::Instant,
) -> eyre::Result<PairedSocket> {
    eyre::ensure!(PAIRING_ACTIVATION_APPROVED, "pairing activation blocked");
    accept_core(listener, java, artifacts, binding, session, until, None).await
}
/// Receipt-gated no-vault preview scope only; no persistent enrollment or production registration.
/// # Errors
/// Rejects an expired consumed receipt, wrong peer, protocol failure, rejection or deadline.
pub async fn accept_preview(
    listener: TcpListener,
    java: &Path,
    artifacts: &Path,
    binding: &PairingBinding,
    session: &Session,
    until: tokio::time::Instant,
    activation: &PreviewActivation,
) -> eyre::Result<PairedSocket> {
    activation.require_live()?;
    accept_core(
        listener,
        java,
        artifacts,
        binding,
        session,
        until,
        Some(activation.trace()),
    )
    .await
}
async fn accept_core(
    listener: TcpListener,
    java: &Path,
    artifacts: &Path,
    binding: &PairingBinding,
    session: &Session,
    until: tokio::time::Instant,
    trace: Option<Arc<PreviewTrace>>,
) -> eyre::Result<PairedSocket> {
    eyre::ensure!(
        super::tailnet_peer::TailnetPeer::permits_bind(listener.local_addr()?),
        "pairing activation blocked"
    );
    let remaining = binding
        .pairing_expires_unix_ms
        .checked_sub(crate::protocol::wire::unix_ms()?)
        .ok_or_else(|| eyre::eyre!("pairing expired"))?;
    let pair_until =
        until.min(tokio::time::Instant::now() + Duration::from_millis(remaining.min(90_000)));
    let (socket, peer) = match tokio::time::timeout_at(pair_until, listener.accept()).await {
        Ok(result) => result?,
        Err(_redacted_error) => {
            emit(trace.as_deref(), PreviewPhase::PairingDeadline);
            eyre::bail!("pairing expired");
        }
    };
    emit(trace.as_deref(), PreviewPhase::PeerAccepted);
    drop(listener); // Even a rejected source consumes the only connection attempt.
    super::tailnet_peer::TailnetPeer::verify_peer(peer).await?;
    emit(trace.as_deref(), PreviewPhase::PeerVerified);
    let mut worker = JavaPairingWorker::start(java, artifacts, binding, until).await?;
    let closed = Arc::new(AtomicBool::new(false));
    let result = tokio::time::timeout_at(
        pair_until,
        handshake(
            socket,
            &mut worker,
            binding,
            session,
            pair_until,
            Arc::clone(&closed),
            (trace.clone(), false, None),
        ),
    )
    .await;
    closed.store(true, Ordering::Release);
    let socket = match result {
        Ok(result) => result?,
        Err(_redacted_error) => {
            emit(trace.as_deref(), PreviewPhase::PairingDeadline);
            eyre::bail!("pairing expired");
        }
    };
    super::tailnet_peer::TailnetPeer::verify_peer(peer).await?;
    emit(trace.as_deref(), PreviewPhase::PeerReverified);
    emit(trace.as_deref(), PreviewPhase::AuthenticatedReady);
    Ok(PairedSocket {
        socket,
        worker,
        until,
    })
}
async fn handshake(
    mut socket: tokio::net::TcpStream,
    worker: &mut JavaPairingWorker,
    binding: &PairingBinding,
    session: &Session,
    pair_until: tokio::time::Instant,
    closed: Arc<AtomicBool>,
    options: (
        Option<Arc<PreviewTrace>>,
        bool,
        Option<Arc<crate::broker::phone::pairing_confirmation::Attempt>>,
    ),
) -> eyre::Result<tokio::net::TcpStream> {
    let (trace, persistent, confirmation) = options;
    let diagnostic = confirmation.as_ref().map(Arc::clone);
    if let Some(d) = &diagnostic {
        d.stage(crate::protocol::phone_status::PhonePhase::PairingHandshake);
    }
    let binding_bytes = binding.encode()?;
    write_frame(&mut socket, binding_bytes.as_bytes()).await?;
    let initial = read_frame(&mut socket).await?;
    worker.call(1, &initial).await?;
    emit(trace.as_deref(), PreviewPhase::ClientInitAccepted);
    let reply = worker.call(2, &[]).await?;
    write_frame(&mut socket, &reply).await?;
    emit(trace.as_deref(), PreviewPhase::ServerInitSent);
    let finish = read_frame(&mut socket).await?;
    worker.call(1, &finish).await?;
    emit(trace.as_deref(), PreviewPhase::ClientFinishAccepted);
    emit(trace.as_deref(), PreviewPhase::HandshakeComplete);
    let code = worker.authentication_code().await?;
    if let Some(attempt) = &confirmation {
        attempt.publish(zeroize::Zeroizing::new(code.to_string()))?;
    }
    let state = PairingPromptState {
        confirmation,
        code,
        until: pair_until.into_std(),
        closed: Arc::clone(&closed),
    };
    if let Some(d) = &diagnostic {
        d.stage(crate::protocol::phone_status::PhonePhase::PairingComparison);
    }
    let prompt_trace = trace.clone();
    let mut prompt = tokio::task::spawn_blocking(move || {
        if persistent {
            super::pairing_prompt::show_persistent(&state)
        } else if let Some(trace) = prompt_trace {
            super::pairing_prompt::show_traced(&state, trace)
        } else {
            super::pairing_prompt::show(&state)
        }
    });
    #[cfg(test)]
    if let Some(d) = &diagnostic {
        d.event("comparison_peek_started");
    }
    let mut peek = [0; 1];
    let agreed = tokio::select! {
            result=&mut prompt=>result.map_err(|_redacted_error|eyre::eyre!("pairing unavailable"))??,
            result=socket.peek(&mut peek)=>{
                match result {Ok(0)=>{#[cfg(test)] if let Some(d)=&diagnostic{d.event("peek_eof");}
    if let Some(d)=&diagnostic{d.failure(crate::protocol::phone_status::PhoneFailureCategory::ReadClosed);}eyre::bail!("pairing peer closed");},Err(ref error)=>{#[cfg(test)] if let Some(d)=&diagnostic{d.event("peek_error");}
    if let Some(d)=&diagnostic{d.transport_failure(error);}eyre::bail!("pairing peer transport");},Ok(_)=>{#[cfg(test)] if let Some(d)=&diagnostic{d.event("peek_peer_data");}}}

                prompt.await.map_err(|_redacted_error|eyre::eyre!("pairing unavailable"))??
            }
        };
    if !agreed {
        if let Some(d) = &diagnostic {
            d.failure(crate::protocol::phone_status::PhoneFailureCategory::PairingRejected);
        }
        emit(trace.as_deref(), PreviewPhase::PairingRejected);
        eyre::bail!("pairing rejected");
    }
    if let Some(d) = &diagnostic {
        d.stage(crate::protocol::phone_status::PhonePhase::PairingLocalConfirmation);
    }
    let confirmation = worker.call(4, &[]).await?;
    write_frame(&mut socket, &confirmation).await?;
    emit(trace.as_deref(), PreviewPhase::LocalConfirmationSent);
    if let Some(d) = &diagnostic {
        d.stage(crate::protocol::phone_status::PhonePhase::PairingPeerConfirmation);
    }
    let peer_confirmation = read_frame(&mut socket).await?;
    emit(trace.as_deref(), PreviewPhase::PeerConfirmationReceived);
    worker.call(5, &peer_confirmation).await?;
    emit(trace.as_deref(), PreviewPhase::PeerConfirmationVerified);
    if let Some(d) = &diagnostic {
        d.stage(crate::protocol::phone_status::PhonePhase::PairingBootstrap);
    }
    let bootstrap = session.transport_key();
    let encrypted = worker.call(6, bootstrap.as_ref()).await?;
    write_frame(&mut socket, &encrypted).await?;
    let ready = read_frame(&mut socket).await?;
    let ready = worker.call(7, &ready).await?;
    eyre::ensure!(
        ready.as_slice() == b"teamy-broker/dummy/ready/v2",
        "pairing rejected"
    );
    Ok(socket)
}

/// Resident-armed exact peer enrollment, separate from legacy temporary activation.
/// # Errors
/// Rejects wrong scope/peer, expired human comparison, protocol failure or rejection.
pub(crate) async fn accept_persistent_socket(
    socket: tokio::net::TcpStream,
    peer: std::net::SocketAddr,
    java: &Path,
    artifacts: &Path,
    binding: &PairingBinding,
    until: tokio::time::Instant,
    confirmation: Arc<crate::broker::phone::pairing_confirmation::Attempt>,
) -> eyre::Result<PairedSocket> {
    eyre::ensure!(
        binding.scope == crate::broker::phone::SCOPE,
        "pairing scope rejected"
    );
    super::tailnet_peer::TailnetPeer::verify_peer(peer).await?;
    let paired = persistent_core(socket, java, artifacts, binding, until, confirmation).await?;
    super::tailnet_peer::TailnetPeer::verify_peer(peer).await?;
    Ok(paired)
}
async fn persistent_core(
    socket: tokio::net::TcpStream,
    java: &Path,
    artifacts: &Path,
    binding: &PairingBinding,
    until: tokio::time::Instant,
    confirmation: Arc<crate::broker::phone::pairing_confirmation::Attempt>,
) -> eyre::Result<PairedSocket> {
    let mut worker = JavaPairingWorker::start(java, artifacts, binding, until).await?;
    let closed = Arc::new(AtomicBool::new(false));
    let _guard = crate::broker::cancel_guard::CancelGuard(Arc::clone(&closed));
    let session = Session::random()?;
    let deadline_diagnostic = Arc::clone(&confirmation);
    let socket = tokio::time::timeout_at(
        until,
        handshake(
            socket,
            &mut worker,
            binding,
            &session,
            until,
            Arc::clone(&closed),
            (None, true, Some(confirmation)),
        ),
    )
    .await
    .map_err(|_redacted_deadline| {
        deadline_diagnostic.failure(crate::protocol::phone_status::PhoneFailureCategory::Deadline);
        eyre::eyre!("pairing deadline")
    })??;
    Ok(PairedSocket {
        socket,
        worker,
        until,
    })
}

#[cfg(test)]
pub(crate) async fn accept_test_loopback_socket(
    socket: tokio::net::TcpStream,
    peer: std::net::SocketAddr,
    java: &Path,
    artifacts: &Path,
    binding: &PairingBinding,
    until: tokio::time::Instant,
    confirmation: Arc<crate::broker::phone::pairing_confirmation::Attempt>,
) -> eyre::Result<PairedSocket> {
    eyre::ensure!(
        peer.ip().is_loopback()
            && socket.local_addr()?.ip().is_loopback()
            && binding.scope == crate::broker::phone::SCOPE,
        "test loopback scope rejected"
    );
    persistent_core(socket, java, artifacts, binding, until, confirmation).await
}
