//! Emits an idle status schema using only synthetic components; never connects to a broker.
#[path = "../tests/support/fake_adapter.rs"]
mod fake_adapter;
#[path = "../tests/support/fake_approval.rs"]
mod fake_approval;
#[path = "../tests/support/fake_storage.rs"]
mod fake_storage;
#[tokio::main]
async fn main() -> eyre::Result<()> {
    use std::io::Write;
    let engine = teamy_broker::broker::engine::Engine::new(
        std::sync::Arc::new(fake_adapter::FakeAdapter::default()),
        std::sync::Arc::new(fake_approval::FakeApproval::default()),
        std::sync::Arc::new(fake_storage::FakeStorage::default()),
    );
    writeln!(
        std::io::stdout().lock(),
        "{}",
        facet_json::to_string_pretty(&engine.status().await?)?
    )?;
    Ok(())
}
