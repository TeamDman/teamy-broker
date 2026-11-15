//! Synthetic foreground helper: injected approval and fake adapter, never invokes op or persists credentials.
use std::env;
use std::time::Duration;
use tokio::time;
#[path = "../tests/support/fake_adapter.rs"]
mod fake_adapter;
#[path = "../tests/support/fake_approval.rs"]
mod fake_approval;
#[path = "../tests/support/fake_storage.rs"]
mod fake_storage;
use std::sync::Arc;
use teamy_broker::broker::engine::Engine;
use teamy_broker::broker::server::serve_at;
#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    eyre::ensure!(
        args.len() == 3 && args[0] == "--synthetic-test-only" && args[1] == "--endpoint",
        "synthetic helper requires explicit test endpoint"
    );
    eyre::ensure!(
        args[2].starts_with(r"\\.\pipe\teamy-broker-test-"),
        "isolated endpoint required"
    );
    let engine = Arc::new(Engine::new(
        Arc::new(fake_adapter::FakeAdapter::default()),
        Arc::new(fake_approval::FakeApproval::default()),
        Arc::new(fake_storage::FakeStorage::default()),
    ));
    let cancel = teamy_cancellation::CancellationToken::new();
    let deadline = cancel.clone();
    tokio::spawn(async move {
        time::sleep(Duration::from_secs(30)).await;
        deadline.request_cancel("synthetic helper deadline");
    });
    serve_at(engine, cancel, &args[2], false).await
}
