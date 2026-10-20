//! Explicit bounded fixture acceptance. No op/vault reference can select the fixture provider.
use std::io::Write;
#[tokio::main]
async fn main() -> eyre::Result<()> {
    if std::env::args().skip(1).collect::<Vec<_>>() != ["--run-isolated-fixture"] {
        writeln!(
            std::io::stdout().lock(),
            "PROPOSAL ONLY: coordinated isolated synthetic credential acceptance; no request started."
        )?;
        return Ok(());
    }
    let client = teamy_broker::sdk::Client::new(None)?;
    let health = client.status().await?;
    eyre::ensure!(
        health.running && health.safe_to_stop && health.active_leases == 0,
        "idle resident required"
    );
    let request = teamy_broker::broker::synthetic_credential_fixture::request();
    client
        .phone_authorize_once(teamy_broker::protocol::Request {
            operation: "phone-authorize-fixture".into(),
            acquire: Some(request.clone()),
            lease_id: None,
        })
        .await?;
    let secret = client.acquire_fixture(request).await?;
    let valid = secret.expose(|value| {
        value == teamy_broker::broker::synthetic_credential_fixture::PROVIDER
            || value == teamy_broker::broker::synthetic_credential_fixture::INPUT
    })?;
    client.purge(Some(secret.lease_id.clone())).await?;
    eyre::ensure!(valid, "fixture result rejected");
    writeln!(
        std::io::stdout().lock(),
        "Isolated fixture resolved; own memory lease purged; value omitted."
    )?;
    Ok(())
}
