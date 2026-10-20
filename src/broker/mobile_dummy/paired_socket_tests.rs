use super::paired_socket::read_frame;
use super::paired_socket::write_frame;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
#[tokio::test]
async fn slow_frame_header_hits_whole_frame_deadline() -> eyre::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let mut client = tokio::net::TcpStream::connect(listener.local_addr()?).await?;
    let (mut server, _) = listener.accept().await?;
    client.write_all(&[0]).await?;
    let started = tokio::time::Instant::now();
    eyre::ensure!(
        read_frame(&mut server).await.is_err(),
        "invalid frame accepted"
    );
    assert!(
        started.elapsed() >= Duration::from_secs(5) && started.elapsed() < Duration::from_secs(7)
    );
    Ok(())
}
#[tokio::test]
async fn bounded_socket_frames_roundtrip_and_reject_oversized() -> eyre::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let mut client = tokio::net::TcpStream::connect(listener.local_addr()?).await?;
    let (mut server, _) = listener.accept().await?;
    write_frame(&mut client, b"TEAMY-DUMMY-ONLY").await?;
    assert!(read_frame(&mut server).await?.as_slice() == b"TEAMY-DUMMY-ONLY");
    client.write_all(&[0, 0, 64, 1]).await?;
    eyre::ensure!(
        read_frame(&mut server).await.is_err(),
        "invalid frame accepted"
    );
    Ok(())
}
