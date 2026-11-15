//! Isolated dummy-only SDK consumer and desktop approval demo.
use std::io;
use std::io::Write;
use std::net::Ipv4Addr;
use std::panic;
use std::process;
use std::sync::Arc;
use std::time::Duration;
use teamy_broker::broker::engine::Engine;
use teamy_broker::broker::mobile_dummy::adapter::MobileDummyAdapter;
use teamy_broker::broker::mobile_dummy::broadcast_approval::BroadcastDummyApproval;
use teamy_broker::broker::mobile_dummy::storage::DummyMemoryStorage;
use teamy_broker::broker::mobile_dummy::{self};
use teamy_broker::broker::server;
use teamy_broker::sdk::Client;
use teamy_broker_handoff::queue::Queue;
use teamy_broker_handoff::session::Session;
use tokio::net::TcpListener;
use tokio::runtime::Builder;
use tokio::task;
use tokio::time;
fn main() {
    panic::set_hook(Box::new(|_| {}));
    if run().is_err() {
        process::exit(1);
    }
}
fn run() -> eyre::Result<()> {
    let session = Session::random()?;
    mobile_dummy::bootstrap::show(&session)?;
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
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 43187)).await?;
            let tcp = tokio::spawn(mobile_dummy::server::serve(listener, queue));
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
            let stop = client.stop_force().await;
            if stop.is_err() {
                ipc.abort();
            } else {
                let _ = ipc.await;
            }
            result
        })
}
