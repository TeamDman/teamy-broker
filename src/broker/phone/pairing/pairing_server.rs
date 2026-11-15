use super::java_pairing_worker::JavaPairingWorker;
use super::paired_socket::PairedSocket;
use super::paired_socket::read_frame;
use super::paired_socket::write_frame;
use super::pairing_prompt::show_persistent;
use super::pairing_prompt_state::PairingPromptState;
use crate::broker::cancel_guard::CancelGuard;
use crate::broker::phone::SCOPE;
use crate::broker::phone::config::PhoneProfile;
use crate::broker::phone::pairing_confirmation::Attempt;
use crate::broker::phone::tailnet::TailnetPeer;
use crate::protocol::phone_status::PhoneFailureCategory;
use crate::protocol::phone_status::PhonePhase;
#[cfg(test)]
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use teamy_broker_handoff::pairing_binding::PairingBinding;
use teamy_broker_handoff::session::Session;
use tokio::net::TcpStream;
use tokio::task;
use tokio::time;
use tokio::time::Instant;
use zeroize::Zeroizing;

async fn handshake(
    mut socket: TcpStream,
    worker: &mut JavaPairingWorker,
    binding: &PairingBinding,
    session: &Session,
    pair_until: Instant,
    closed: Arc<AtomicBool>,
    confirmation: Arc<Attempt>,
) -> eyre::Result<TcpStream> {
    confirmation.stage(PhonePhase::PairingHandshake);
    let binding_bytes = binding.encode()?;
    write_frame(&mut socket, binding_bytes.as_bytes()).await?;
    let initial = read_frame(&mut socket).await?;
    worker.call(1, &initial).await?;
    let reply = worker.call(2, &[]).await?;
    write_frame(&mut socket, &reply).await?;
    let finish = read_frame(&mut socket).await?;
    worker.call(1, &finish).await?;
    let code = worker.authentication_code().await?;
    confirmation.publish(Zeroizing::new(code.to_string()))?;
    let state = PairingPromptState {
        confirmation: Some(Arc::clone(&confirmation)),
        code,
        until: pair_until.into_std(),
        closed,
    };
    confirmation.stage(PhonePhase::PairingComparison);
    let mut prompt = task::spawn_blocking(move || show_persistent(&state));
    #[cfg(test)]
    confirmation.event("comparison_peek_started");
    let mut peek = [0; 1];
    let agreed = tokio::select! {
        result = &mut prompt => result.map_err(|_redacted| eyre::eyre!("pairing unavailable"))??,
        result = socket.peek(&mut peek) => {
            match result {
                Ok(0) => {
                    #[cfg(test)]
                    confirmation.event("peek_eof");
                    confirmation.failure(PhoneFailureCategory::ReadClosed);
                    eyre::bail!("pairing peer closed");
                }
                Err(ref error) => {
                    #[cfg(test)]
                    confirmation.event("peek_error");
                    confirmation.transport_failure(error);
                    eyre::bail!("pairing peer transport");
                }
                Ok(_) => {
                    #[cfg(test)]
                    confirmation.event("peek_peer_data");
                }
            }
            prompt.await.map_err(|_redacted| eyre::eyre!("pairing unavailable"))??
        }
    };
    if !agreed {
        confirmation.failure(PhoneFailureCategory::PairingRejected);
        eyre::bail!("pairing rejected");
    }
    confirmation.stage(PhonePhase::PairingLocalConfirmation);
    let local_confirmation = worker.call(4, &[]).await?;
    write_frame(&mut socket, &local_confirmation).await?;
    confirmation.stage(PhonePhase::PairingPeerConfirmation);
    let peer_confirmation = read_frame(&mut socket).await?;
    worker.call(5, &peer_confirmation).await?;
    confirmation.stage(PhonePhase::PairingBootstrap);
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

/// Resident-armed exact peer enrollment.
/// # Errors
/// Rejects wrong scope/peer, expired human comparison, protocol failure or rejection.
pub(crate) async fn accept_persistent_socket(
    socket: TcpStream,
    java: &Path,
    artifacts: &Path,
    binding: &PairingBinding,
    until: Instant,
    confirmation: Arc<Attempt>,
    profile: &PhoneProfile,
) -> eyre::Result<PairedSocket> {
    let peer = socket.peer_addr()?;
    eyre::ensure!(binding.scope == SCOPE, "pairing scope rejected");
    TailnetPeer::verify_peer_for(peer, profile).await?;
    let paired = persistent_core(socket, java, artifacts, binding, until, confirmation).await?;
    TailnetPeer::verify_peer_for(peer, profile).await?;
    Ok(paired)
}
async fn persistent_core(
    socket: TcpStream,
    java: &Path,
    artifacts: &Path,
    binding: &PairingBinding,
    until: Instant,
    confirmation: Arc<Attempt>,
) -> eyre::Result<PairedSocket> {
    let mut worker = JavaPairingWorker::start(java, artifacts, binding, until).await?;
    let closed = Arc::new(AtomicBool::new(false));
    let _guard = CancelGuard(Arc::clone(&closed));
    let session = Session::random()?;
    let deadline_diagnostic = Arc::clone(&confirmation);
    let socket = time::timeout_at(
        until,
        handshake(
            socket,
            &mut worker,
            binding,
            &session,
            until,
            Arc::clone(&closed),
            confirmation,
        ),
    )
    .await
    .map_err(|_redacted_deadline| {
        deadline_diagnostic.failure(PhoneFailureCategory::Deadline);
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
    socket: TcpStream,
    peer: SocketAddr,
    java: &Path,
    artifacts: &Path,
    binding: &PairingBinding,
    until: Instant,
    confirmation: Arc<Attempt>,
) -> eyre::Result<PairedSocket> {
    eyre::ensure!(
        peer.ip().is_loopback()
            && socket.local_addr()?.ip().is_loopback()
            && binding.scope == SCOPE,
        "test loopback scope rejected"
    );
    persistent_core(socket, java, artifacts, binding, until, confirmation).await
}
