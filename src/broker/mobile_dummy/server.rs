use std::sync::Arc;
use std::time::Duration;
use teamy_broker_handoff::queue::Queue;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
/// This API cannot expose a non-loopback listener. No configurable production bind address.
/// # Errors
/// Rejects non-loopback listeners and reports listener I/O failure.
pub async fn serve(listener: TcpListener, queue: Arc<Queue>) -> eyre::Result<()> {
    eyre::ensure!(
        listener.local_addr()?.ip().is_loopback(),
        "loopback required"
    );
    let permits = Arc::new(tokio::sync::Semaphore::new(8));
    loop {
        let (mut socket, peer) = listener.accept().await?;
        if !peer.ip().is_loopback() {
            continue;
        }
        let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else {
            continue;
        };
        let queue = Arc::clone(&queue);
        tokio::spawn(async move {
            let _permit = permit;
            // Never log raw errors, metadata, ciphertext or secret values.
            let _ =
                tokio::time::timeout(Duration::from_secs(5), exchange(&mut socket, &queue)).await;
        });
    }
}
pub(super) async fn exchange<S: AsyncRead + AsyncWrite + Unpin>(
    socket: &mut S,
    queue: &Queue,
) -> eyre::Result<()> {
    let packet: super::packet::Packet = crate::protocol::wire::read(socket).await?;
    let result = match packet.operation.as_str() {
        "poll" => queue.poll(&packet.envelope)?,
        "submit" => queue.submit(&packet.envelope)?,
        _ => eyre::bail!("invalid operation"),
    };
    crate::protocol::wire::write(socket, &result).await?;
    socket.flush().await?;
    Ok(())
}
