//! Isolated resident for automated native presentation regression, never a production daemon.
use std::env;
use std::io;
use std::io::Write;
use std::panic;
use teamy_broker::broker::engine::Engine;
use teamy_broker::broker::native_approval::NativeApproval;
use teamy_broker::broker::server;
use tokio::runtime::Builder;
#[path = "../tests/support/fake_adapter.rs"]
mod fake_adapter;
#[path = "../tests/support/fake_storage.rs"]
mod fake_storage;
use std::sync::Arc;
use std::sync::atomic::Ordering;
fn main() -> eyre::Result<()> {
    panic::set_hook(Box::new(|_| {}));
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 1 {
        Write::write_all(&mut io::stdout().lock(),b"PROPOSAL ONLY: isolated native resident fixture. Requires one generated teamy-broker-test-presentation local pipe; no real provider.\n")?;
        return Ok(());
    }
    let endpoint = &args[0];
    let id = endpoint
        .strip_prefix(r"\\.\pipe\teamy-broker-test-presentation-")
        .ok_or_else(|| eyre::eyre!("invalid isolated test endpoint"))?;
    uuid::Uuid::parse_str(id)
        .map_err(|_redacted_error| eyre::eyre!("invalid isolated test endpoint"))?;
    let adapter = Arc::new(fake_adapter::FakeAdapter::default());
    let storage = Arc::new(fake_storage::FakeStorage::default());
    let engine = Arc::new(Engine::new(
        Arc::<fake_adapter::FakeAdapter>::clone(&adapter),
        Arc::new(NativeApproval),
        Arc::<fake_storage::FakeStorage>::clone(&storage),
    ));
    let runtime = Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    runtime.block_on(server::serve_at(
        engine,
        teamy_cancellation::CancellationToken::new(),
        endpoint,
        false,
    ))?;
    eyre::ensure!(
        adapter.calls.load(Ordering::SeqCst) == 0,
        "preview reached synthetic adapter"
    );
    eyre::ensure!(
        storage
            .published
            .lock()
            .expect("synthetic storage lock")
            .is_empty()
            && storage.removals.load(Ordering::SeqCst) == 0,
        "preview reached synthetic storage"
    );
    Ok(())
}
