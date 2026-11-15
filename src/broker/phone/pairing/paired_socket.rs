use super::java_pairing_worker::JavaPairingWorker;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time;
use tokio::time::Instant;
use zeroize::Zeroizing;
/// Single authenticated live connection; no request can move to another socket or session.
pub struct PairedSocket {
    pub(super) socket: TcpStream,
    pub(super) worker: JavaPairingWorker,
    pub(super) until: Instant,
}
impl Debug for PairedSocket {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("PairedSocket([redacted])")
    }
}
pub(super) async fn read_frame(socket: &mut TcpStream) -> eyre::Result<Zeroizing<Vec<u8>>> {
    let first = socket
        .read_u8()
        .await
        .map_err(|_redacted_error| eyre::eyre!("session unavailable"))?;
    // Idle foreground socket may wait for manual Refresh; a frame's first byte starts a hard 5s timer.
    time::timeout(Duration::from_secs(5), async {
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
