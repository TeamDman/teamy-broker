use super::Controller;
use super::message::Message;
use crate::protocol::phone_status::PhoneFailureCategory as F;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
/// Idle waiting does not consume frame time. Once a byte arrives, the entire frame is bounded.
pub(super) async fn read<R: AsyncRead + Unpin>(
    stream: &mut R,
    controller: &Controller,
    idle: std::time::Duration,
) -> eyre::Result<Message> {
    let first = match tokio::time::timeout(idle, stream.read_u8()).await {
        Ok(Ok(first)) => first,
        Err(_) => {
            controller.failure(F::Deadline);
            eyre::bail!("phone idle deadline")
        }
        Ok(Err(error)) => {
            controller.failure(if error.kind() == std::io::ErrorKind::UnexpectedEof {
                F::ReadClosed
            } else {
                F::ReadTransport
            });
            eyre::bail!("phone read unavailable")
        }
    };
    match tokio::time::timeout(super::timing::FRAME, frame(stream, first)).await {
        Ok(Ok(message)) => Ok(message),
        Err(_) => {
            controller.failure(F::Deadline);
            eyre::bail!("phone frame deadline")
        }
        Ok(Err(error)) => {
            controller.failure(match error.downcast_ref::<std::io::Error>() {
                Some(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => F::ReadClosed,
                Some(_) => F::ReadTransport,
                None => F::MessageRejected,
            });
            eyre::bail!("phone frame unavailable")
        }
    }
}
async fn frame<R: AsyncRead + Unpin>(stream: &mut R, first: u8) -> eyre::Result<Message> {
    let mut header = [first, 0, 0, 0];
    stream.read_exact(&mut header[1..]).await?;
    let length = u32::from_be_bytes(header) as usize;
    eyre::ensure!(length <= 16384, "oversized metadata");
    let mut bytes = zeroize::Zeroizing::new(vec![0; length]);
    stream.read_exact(&mut bytes).await?;
    let value = std::str::from_utf8(&bytes).map_err(|_redacted| eyre::eyre!("invalid metadata"))?;
    facet_json::from_str(value).map_err(|_redacted| eyre::eyre!("invalid metadata"))
}
