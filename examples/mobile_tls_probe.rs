//! Finite isolated loopback handshake probe for the exact Android/JVM TLS client helper.
use std::io;
use std::io::Write;
use std::net::Ipv4Addr;
use std::panic;
use std::process;
use std::time::Duration;
use teamy_broker::broker::mobile_dummy::tls_identity::TlsIdentity;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::runtime::Builder;
use tokio::time;
fn main() {
    panic::set_hook(Box::new(|_| {}));
    if run().is_err() {
        process::exit(1);
    }
}
fn run() -> eyre::Result<()> {
    Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let identity = TlsIdentity::random()?;
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
            let public = format!(
                "{} {}\n",
                listener.local_addr()?.port(),
                identity.fingerprint
            );
            Write::write_all(&mut io::stdout().lock(), public.as_bytes())?;
            Write::flush(&mut io::stdout().lock())?;
            time::timeout(Duration::from_secs(5), async {
                let (socket, peer) = listener.accept().await?;
                eyre::ensure!(peer.ip().is_loopback(), "isolated probe required");
                // A deliberately wrong-pinned client can reject TLS; no plaintext fallback.
                if let Ok(mut tls) = identity.acceptor.accept(socket).await {
                    let length = tls.read_u32().await?;
                    eyre::ensure!(
                        length == u32::try_from(teamy_broker_handoff::DUMMY_SECRET.len())?,
                        "dummy frame required"
                    );
                    let mut bytes = zeroize::Zeroizing::new(vec![0; usize::try_from(length)?]);
                    tls.read_exact(&mut bytes).await?;
                    eyre::ensure!(
                        bytes.as_slice() == teamy_broker_handoff::DUMMY_SECRET,
                        "dummy value required"
                    );
                    tls.write_u32(length).await?;
                    tls.write_all(&bytes).await?;
                    tls.flush().await?;
                }
                Ok::<_, eyre::Report>(())
            })
            .await
            .map_err(|_redacted_error| eyre::eyre!("isolated probe timeout"))??;
            Ok(())
        })
}
