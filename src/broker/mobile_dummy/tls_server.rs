use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use teamy_broker_handoff::queue::Queue;
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tokio::time;
use tokio::time::Instant;
/// Finite loopback TLS fixture; the synthetic queue owns application authentication.
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
    serve_until(listener, queue, acceptor, Instant::now() + lifetime).await
}
/// Use an absolute monotonic deadline anchored when the listener is bound.
/// # Errors
/// Rejects unapproved/mismatched binds and expired or unbounded deadlines.
pub async fn serve_until(
    listener: TcpListener,
    queue: Arc<Queue>,
    acceptor: tokio_rustls::TlsAcceptor,
    until: Instant,
) -> eyre::Result<()> {
    let local = listener.local_addr()?;
    require_loopback(local)?;
    eyre::ensure!(
        until > Instant::now() && until <= Instant::now() + Duration::from_mins(5),
        "finite dummy lifetime required"
    );
    let mut workers = JoinSet::new();
    loop {
        tokio::select! {
            biased;
            () = time::sleep_until(until) => break,
            finished = workers.join_next(), if !workers.is_empty() => { let _ = finished; }
            incoming = listener.accept() => {
                let (socket, peer) = incoming?;
                if workers.len() >= 4 || !peer.ip().is_loopback() { continue; }
                let queue = Arc::clone(&queue); let acceptor = acceptor.clone();
                workers.spawn(async move {
                    let _ = time::timeout_at(until.min(Instant::now()+Duration::from_secs(5)), async {
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

fn require_loopback(address: SocketAddr) -> eyre::Result<()> {
    eyre::ensure!(
        address.ip().is_loopback(),
        "synthetic TLS requires loopback"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::require_loopback;
    use std::net::Ipv4Addr;
    use std::net::Ipv6Addr;

    #[test]
    fn synthetic_tls_rejects_non_loopback_without_binding_a_socket() {
        for address in [
            (Ipv4Addr::UNSPECIFIED, 43188).into(),
            (Ipv4Addr::BROADCAST, 43188).into(),
            (Ipv6Addr::UNSPECIFIED, 43188).into(),
        ] {
            assert_eq!(
                require_loopback(address).unwrap_err().to_string(),
                "synthetic TLS requires loopback"
            );
        }
        for address in [
            (Ipv4Addr::LOCALHOST, 43188).into(),
            (Ipv6Addr::LOCALHOST, 43188).into(),
        ] {
            require_loopback(address).unwrap();
        }
    }
}
