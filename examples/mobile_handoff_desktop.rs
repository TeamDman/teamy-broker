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
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let listener =
                tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 43187)).await?;
            let tcp = tokio::spawn(mobile_dummy::server::serve(listener, queue));
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
            let stop = client.stop_force().await;
            if stop.is_err() {
                ipc.abort();
            } else {
                let _ = ipc.await;
            }
            result
        })
}
