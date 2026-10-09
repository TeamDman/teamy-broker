//! Synthetic foreground helper: injected approval and fake adapter, never invokes op or persists credentials.
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
    let args: Vec<_> = std::env::args().skip(1).collect();
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
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        deadline.request_cancel("synthetic helper deadline");
    });
    serve_at(engine, cancel, &args[2], false).await
}
