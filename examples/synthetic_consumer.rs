//! End-to-end isolated consumer using only synthetic credentials and injected approval.
#[path = "../tests/support/mod.rs"]
mod support;
use std::io::Write;
use std::sync::Arc;
use teamy_broker::broker::engine::Engine;
use teamy_broker::broker::server::serve_at;
use teamy_broker::sdk::Backend;
use teamy_broker::sdk::Client;
use teamy_broker::sdk::resolve_provider;
#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let cli = (args.len() == 2 && args[0] == "--provider").then(|| args[1].as_str());
    let environment = std::env::var("TEAMY_SYNTHETIC_CREDENTIAL_PROVIDER").ok();
    let provider = resolve_provider(cli, environment.as_deref())?;
    eyre::ensure!(
        provider == "teamy-broker",
        "synthetic example supports explicit teamy-broker memory only"
    );
    let endpoint = format!(r"\\.\pipe\teamy-broker-test-{}", uuid::Uuid::new_v4());
    let engine = Arc::new(Engine::new(
        Arc::new(support::fake_adapter::FakeAdapter::default()),
        Arc::new(support::fake_approval::FakeApproval::default()),
        Arc::new(support::fake_storage::FakeStorage::default()),
    ));
    let cancel = teamy_cancellation::CancellationToken::new();
    let server = {
        let endpoint = endpoint.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move { serve_at(engine, cancel, &endpoint, false).await })
    };
    let client = Client::new(None)?.with_endpoint(endpoint);
    while !client.status().await?.running {
        tokio::task::yield_now().await;
    }
    let secret = client.acquire(support::request(Backend::Memory)).await?;
    let length = secret.expose(<[u8]>::len)?;
    writeln!(
        std::io::stdout().lock(),
        "Synthetic credential consumed ({length} bytes); secret value omitted"
    )?;
    client.purge(Some(secret.lease_id.clone())).await?;
    client.stop_force().await?;
    server.await??;
    Ok(())
}
