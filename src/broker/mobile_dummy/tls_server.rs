use std::sync::Arc;
use std::time::Duration;
use teamy_broker_handoff::queue::Queue;
use tokio::net::TcpListener;
/// Finite, bounded TLS listener; synthetic queue is still the application authority.
/// # Errors
/// Rejects unapproved/mismatched binds, out-of-range lifetimes and listener errors.
pub async fn serve(
    listener: TcpListener,
    queue: Arc<Queue>,
    acceptor: tokio_rustls::TlsAcceptor,
    lifetime: Duration,
) -> eyre::Result<()> {
    eyre::ensure!(
        lifetime > Duration::ZERO && lifetime <= Duration::from_mins(5),
        "finite dummy lifetime required"
    );
    serve_until(
        listener,
        queue,
        acceptor,
        tokio::time::Instant::now() + lifetime,
    )
    .await
}
/// Use an absolute monotonic deadline anchored when the listener is bound.
/// # Errors
/// Rejects unapproved/mismatched binds and expired or unbounded deadlines.
pub async fn serve_until(
    listener: TcpListener,
    queue: Arc<Queue>,
    acceptor: tokio_rustls::TlsAcceptor,
    until: tokio::time::Instant,
) -> eyre::Result<()> {
    let local = listener.local_addr()?;
    let loopback = local.ip().is_loopback();
    eyre::ensure!(
        until > tokio::time::Instant::now()
            && until <= tokio::time::Instant::now() + Duration::from_mins(5),
        "finite dummy lifetime required"
    );
    if !loopback {
        eyre::ensure!(
            super::tailnet_peer::ACTIVATION_APPROVED
                && super::tailnet_peer::TailnetPeer::permits_bind(local),
            "tailnet activation blocked"
        );
        super::tailnet_peer::TailnetPeer::verify_desktop().await?;
    }
    let mut workers = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            biased;
            () = tokio::time::sleep_until(until) => break,
            finished = workers.join_next(), if !workers.is_empty() => { let _ = finished; }
            incoming = listener.accept() => {
                let (socket, peer) = incoming?;
                if workers.len() >= 4 || (loopback && !peer.ip().is_loopback())
                    || (!loopback && !super::tailnet_peer::TailnetPeer::permits_source(peer)) { continue; }
                let queue = Arc::clone(&queue); let acceptor = acceptor.clone();
                workers.spawn(async move {
                    let _ = tokio::time::timeout_at(until.min(tokio::time::Instant::now()+Duration::from_secs(5)), async {
                        if !loopback { super::tailnet_peer::TailnetPeer::verify_peer(peer).await?; }
                        let mut tls = acceptor.accept(socket).await
                            .map_err(|_redacted_error| eyre::eyre!("dummy TLS rejected"))?;
                        // No unauthenticated packet can resolve the Queue; existing AEAD is mandatory.
                        super::server::exchange(&mut tls, &queue).await
                    }).await;
                });
            }
        }
    }
    workers.shutdown().await;
    Ok(())
}
