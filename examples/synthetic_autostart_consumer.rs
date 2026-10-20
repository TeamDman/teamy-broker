//! Synthetic-only capture regression helper. Leaves its isolated fake daemon running for the parent test to stop.
use std::io::Write;
use teamy_broker::sdk::Backend;
use teamy_broker::sdk::Client;
#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    eyre::ensure!(
        args.len() == 2 && args[1].starts_with(r"\\.\pipe\teamy-broker-test-"),
        "isolated synthetic endpoint required"
    );
    let client = Client::new(Some(args[0].clone().into()))?
        .with_endpoint(args[1].clone())
        .with_start_arguments(vec![
            "--synthetic-test-only".into(),
            "--endpoint".into(),
            args[1].clone(),
        ]);
    let secret = client
        .acquire(teamy_broker::sdk::AcquireRequest {
            account: "synthetic-account".into(),
            secret_ref: "op://Synthetic/Bot/credential".into(),
            purpose: "synthetic capture test".into(),
            requesting_app: "synthetic-consumer".into(),
            destination: "https://example.invalid".into(),
            backend: Backend::Memory,
        })
        .await?;
    secret.expose(|_| ())?;
    writeln!(std::io::stdout().lock(), "synthetic consumer complete")?;
    Ok(())
}
