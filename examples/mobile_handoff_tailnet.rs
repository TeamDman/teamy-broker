//! Isolated dummy-only SDK consumer and desktop approval demo.
use std::sync::Arc;
use std::time::Duration;
use teamy_broker::broker::engine::Engine;
use teamy_broker::broker::mobile_dummy::adapter::MobileDummyAdapter;
use teamy_broker::broker::mobile_dummy::broadcast_approval::BroadcastDummyApproval;
use teamy_broker::broker::mobile_dummy::storage::DummyMemoryStorage;
use teamy_broker::broker::mobile_dummy::{self};
use teamy_broker::sdk::Client;
use teamy_broker_handoff::queue::Queue;
use teamy_broker_handoff::session::Session;
fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    if run().is_err() {
        std::process::exit(1);
    }
}
fn run() -> eyre::Result<()> {
    use teamy_broker::broker::mobile_dummy::tailnet_peer::ACTIVATION_APPROVED;
    use teamy_broker::broker::mobile_dummy::tailnet_peer::DESKTOP;
    use teamy_broker::broker::mobile_dummy::tailnet_peer::PORT;
    use teamy_broker::broker::mobile_dummy::tailnet_peer::TailnetPeer;
    if std::env::args_os().skip(1).collect::<Vec<_>>()
        != [std::ffi::OsString::from("--activate-dummy-tailnet")]
    {
        std::io::Write::write_all(&mut std::io::stdout().lock(), b"DUMMY ONLY: configured exact TLS endpoint and peer, 180-second limit. Await phone readiness and explicit launch; no listener started.\n")?;
        return Ok(());
    }
    eyre::ensure!(ACTIVATION_APPROVED, "tailnet activation blocked");
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(TailnetPeer::verify_desktop())?;
    let identity = mobile_dummy::tls_identity::TlsIdentity::random()?;
    let session = Session::random()?;
    mobile_dummy::bootstrap::show_with_tls(&session, &identity.fingerprint)?;
    let queue = Arc::new(Queue::new(session));
    let engine = Arc::new(Engine::new(
        Arc::new(MobileDummyAdapter),
        Arc::new(BroadcastDummyApproval {
            queue: Arc::clone(&queue),
        }),
        Arc::new(DummyMemoryStorage),
    ));
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let listener = tokio::net::TcpListener::bind((DESKTOP, PORT)).await?;
            let until = tokio::time::Instant::now() + Duration::from_mins(3);
            let expiry = chrono::Utc::now() + chrono::Duration::minutes(3);
            // Only public endpoint/deadline status is emitted. Key/pin remain in the native dialog.
            let status = format!("Dummy TLS listener bound at {DESKTOP}:{PORT}; permitted phone {}; expires {} UTC at latest.\n", teamy_broker::broker::mobile_dummy::tailnet_peer::PHONE, expiry.format("%Y-%m-%d %H:%M:%S%.3f"));
            std::io::Write::write_all(&mut std::io::stdout().lock(), status.as_bytes())?;
            std::io::Write::flush(&mut std::io::stdout().lock())?;
            let tcp = tokio::spawn(mobile_dummy::tls_server::serve_until(
                listener,
                queue,
                identity.acceptor,
                until,
            ));
            let endpoint = format!(
                r"\\.\pipe\teamy-broker-mobile-dummy-{}",
                uuid::Uuid::new_v4()
            );
            // Random test endpoint and reconcile=false keep existing daemon/cache state untouched.
            let ipc_endpoint = endpoint.clone();
            let ipc = tokio::spawn(async move {
                teamy_broker::broker::server::serve_at(
                    engine,
                    teamy_cancellation::CancellationToken::new(),
                    &ipc_endpoint,
                    false,
                )
                .await
            });
            let client = Client::new(None)?
                .with_endpoint(endpoint)
                .with_deadlines(Duration::from_secs(2), Duration::from_secs(65));
            let result = async {
                tokio::time::timeout(Duration::from_secs(3), async {
                    while !client.status().await?.running {
                        tokio::task::yield_now().await;
                    }
                    Ok::<_, eyre::Report>(())
                })
                .await
                .map_err(|_redacted_error| eyre::eyre!("dummy endpoint unavailable"))??;
                let secret = client.acquire(mobile_dummy::request()).await?;
                eyre::ensure!(
                    secret.expose(|bytes| bytes == teamy_broker_handoff::DUMMY_SECRET)?,
                    "dummy grant required"
                );
                client.purge(Some(secret.lease_id.clone())).await?;
                drop(secret);
                let mut out = std::io::stdout().lock();
                std::io::Write::write_all(
                    &mut out,
                    b"Dummy SDK handoff completed; memory lease purged.\n",
                )?;
                Ok::<_, eyre::Report>(())
            }
            .await;
            tcp.abort();
            let _ = tcp.await;
            let stopped = format!("Dummy TLS listener stopped at {} UTC.\n", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"));
            std::io::Write::write_all(&mut std::io::stdout().lock(), stopped.as_bytes())?;
            std::io::Write::flush(&mut std::io::stdout().lock())?;
            let stop = client.stop_force().await;
            if stop.is_err() {
                ipc.abort();
            } else {
                let _ = ipc.await;
            }
            result
        })
}
