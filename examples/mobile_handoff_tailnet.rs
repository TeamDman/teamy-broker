//! Isolated dummy-only SDK consumer and desktop approval demo.
use std::env;
use std::ffi::OsString;
use std::io;
use std::io::Write;
use std::panic;
use std::process;
use std::sync::Arc;
use std::time::Duration;
use teamy_broker::broker::engine::Engine;
use teamy_broker::broker::mobile_dummy::adapter::MobileDummyAdapter;
use teamy_broker::broker::mobile_dummy::broadcast_approval::BroadcastDummyApproval;
use teamy_broker::broker::mobile_dummy::storage::DummyMemoryStorage;
use teamy_broker::broker::mobile_dummy::tailnet_peer::PORT;
use teamy_broker::broker::mobile_dummy::tailnet_peer::TailnetPeer;
use teamy_broker::broker::mobile_dummy::{self};
use teamy_broker::broker::server;
use teamy_broker::paths::AppHome;
use teamy_broker::sdk::Client;
use teamy_broker_handoff::queue::Queue;
use teamy_broker_handoff::session::Session;
use tokio::net::TcpListener;
use tokio::runtime::Builder;
use tokio::task;
use tokio::time;
use tokio::time::Instant;
fn main() {
    panic::set_hook(Box::new(|_| {}));
    if run().is_err() {
        process::exit(1);
    }
}
fn run() -> eyre::Result<()> {
    if env::args_os().skip(1).collect::<Vec<_>>() != [OsString::from("--activate-dummy-tailnet")] {
        Write::write_all(&mut io::stdout().lock(), b"DUMMY ONLY: configured exact TLS endpoint and peer, 180-second limit. Await phone readiness and explicit launch; no listener started.\n")?;
        return Ok(());
    }
    let profile = mobile_dummy::phone_profile::PhoneProfile::load(&AppHome::resolve()?)?;
    Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(TailnetPeer::verify_desktop_for(&profile))?;
    let identity = mobile_dummy::tls_identity::TlsIdentity::for_address(profile.desktop.get())?;
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
    Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let listener = TcpListener::bind((profile.desktop.get(), PORT)).await?;
            let until = Instant::now() + Duration::from_mins(3);
            let expiry = chrono::Utc::now() + chrono::Duration::minutes(3);
            // Only public endpoint/deadline status is emitted. Key/pin remain in the native dialog.
            let status = format!(
                "Dummy TLS listener bound; expires {} UTC at latest.\n",
                expiry.format("%Y-%m-%d %H:%M:%S%.3f")
            );
            Write::write_all(&mut io::stdout().lock(), status.as_bytes())?;
            Write::flush(&mut io::stdout().lock())?;
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
                server::serve_at(
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
                time::timeout(Duration::from_secs(3), async {
                    while !client.status().await?.running {
                        task::yield_now().await;
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
                let mut out = io::stdout().lock();
                Write::write_all(
                    &mut out,
                    b"Dummy SDK handoff completed; memory lease purged.\n",
                )?;
                Ok::<_, eyre::Report>(())
            }
            .await;
            tcp.abort();
            let _ = tcp.await;
            let stopped = format!(
                "Dummy TLS listener stopped at {} UTC.\n",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f")
            );
            Write::write_all(&mut io::stdout().lock(), stopped.as_bytes())?;
            Write::flush(&mut io::stdout().lock())?;
            let stop = client.stop_force().await;
            if stop.is_err() {
                ipc.abort();
            } else {
                let _ = ipc.await;
            }
            result
        })
}
