use super::java_pairing_worker::JavaPairingWorker;
use super::packet::Packet;
use std::sync::Arc;
use teamy_broker_handoff::queue::Queue;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use zeroize::Zeroizing;
/// Single authenticated live connection; no request can move to another socket or session.
pub struct PairedSocket {
    pub(super) socket: TcpStream,
    pub(super) worker: JavaPairingWorker,
    pub(super) until: tokio::time::Instant,
}
impl std::fmt::Debug for PairedSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PairedSocket([redacted])")
    }
}
impl PairedSocket {
    /// # Errors
    /// Returns static redacted errors on timeout, invalid frames, authentication or queue rejection.
    pub async fn serve(mut self, queue: Arc<Queue>) -> eyre::Result<()> {
        loop {
            let encrypted = tokio::time::timeout_at(self.until, read_frame(&mut self.socket))
                .await
                .map_err(|_redacted_error| eyre::eyre!("session expired"))??;
            let operation_until = self
                .until
                .min(tokio::time::Instant::now() + std::time::Duration::from_secs(5));
            tokio::time::timeout_at(operation_until, async {
                let plaintext = self.worker.call(7, &encrypted).await?;
                let text = std::str::from_utf8(&plaintext)
                    .map_err(|_redacted_error| eyre::eyre!("packet rejected"))?;
                let packet: Packet = facet_json::from_str(text)
                    .map_err(|_redacted_error| eyre::eyre!("packet rejected"))?;
                let envelope = match packet.operation.as_str() {
                    "poll" => queue.poll(&packet.envelope)?,
                    "submit" => queue.submit(&packet.envelope)?,
                    _ => eyre::bail!("packet rejected"),
                };
                let response = Zeroizing::new(
                    facet_json::to_string(&envelope)
                        .map_err(|_redacted_error| eyre::eyre!("packet rejected"))?,
                );
                let frame = self.worker.call(6, response.as_bytes()).await?;
                write_frame(&mut self.socket, &frame).await
            })
            .await
            .map_err(|_redacted_error| eyre::eyre!("session expired"))??;
        }
    }
}
pub(super) async fn read_frame(socket: &mut TcpStream) -> eyre::Result<Zeroizing<Vec<u8>>> {
    let first = socket
        .read_u8()
        .await
        .map_err(|_redacted_error| eyre::eyre!("session unavailable"))?;
    // Idle foreground socket may wait for manual Refresh; a frame's first byte starts a hard 5s timer.
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let mut header = [first, 0, 0, 0];
        socket.read_exact(&mut header[1..]).await?;
        let size = u32::from_be_bytes(header);
        eyre::ensure!((1..=16_384).contains(&size), "frame rejected");
        let mut frame = Zeroizing::new(vec![0; usize::try_from(size)?]);
        socket.read_exact(&mut frame).await?;
        Ok::<_, eyre::Report>(frame)
    })
    .await
    .map_err(|_redacted_error| eyre::eyre!("frame expired"))?
}
pub(super) async fn write_frame(socket: &mut TcpStream, frame: &[u8]) -> eyre::Result<()> {
    eyre::ensure!(!frame.is_empty() && frame.len() <= 16_384, "frame rejected");
    socket.write_u32(u32::try_from(frame.len())?).await?;
    socket.write_all(frame).await?;
    socket.flush().await?;
    Ok(())
}
