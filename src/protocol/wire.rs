use facet::Facet;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use zeroize::Zeroizing;
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub async fn write<T: Facet<'static>, W: AsyncWrite + Unpin>(
    stream: &mut W,
    value: &T,
) -> eyre::Result<()> {
    let bytes =
        facet_json::to_string(value).map_err(|_redacted_error| eyre::eyre!("encode metadata"))?;
    eyre::ensure!(bytes.len() <= 16384, "oversized metadata");
    stream.write_u32(u32::try_from(bytes.len())?).await?;
    stream.write_all(bytes.as_bytes()).await?;
    Ok(())
}
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub async fn read<T: Facet<'static>, R: AsyncRead + Unpin>(stream: &mut R) -> eyre::Result<T> {
    let len = stream.read_u32().await? as usize;
    eyre::ensure!(len <= 16384, "oversized metadata");
    let mut bytes = vec![0; len];
    stream.read_exact(&mut bytes).await?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_redacted_error| eyre::eyre!("invalid metadata"))?;
    facet_json::from_str(text).map_err(|_redacted_error| eyre::eyre!("invalid metadata"))
}
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub async fn read_secret<R: AsyncRead + Unpin>(stream: &mut R) -> eyre::Result<Zeroizing<Vec<u8>>> {
    let len = stream.read_u32().await? as usize;
    eyre::ensure!(len <= super::MAX_SECRET, "oversized secret");
    let mut bytes = Zeroizing::new(vec![0; len]);
    stream.read_exact(&mut bytes).await?;
    Ok(bytes)
}
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub async fn write_secret<W: AsyncWrite + Unpin>(stream: &mut W, bytes: &[u8]) -> eyre::Result<()> {
    eyre::ensure!(bytes.len() <= super::MAX_SECRET, "oversized secret");
    stream.write_u32(u32::try_from(bytes.len())?).await?;
    stream.write_all(bytes).await?;
    stream.flush().await?;
    Ok(())
}
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn unix_ms() -> eyre::Result<u64> {
    Ok(u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis(),
    )?)
}
