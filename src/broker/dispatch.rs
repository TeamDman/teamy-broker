use super::app_request::AppRequest;
use crate::cli::output::CliOutput;
use crate::sdk::Client;
use std::sync::Arc;
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub async fn invoke(
    request: AppRequest,
    cancellation: teamy_cancellation::CancellationToken,
) -> eyre::Result<CliOutput> {
    cancellation.bail_if_cancelled()?;
    match request {
        AppRequest::Serve(executable) => {
            let engine = Arc::new(super::engine::Engine::new(
                Arc::new(super::one_password::OnePassword { executable }),
                Arc::new(super::native_approval::NativeApproval),
                Arc::new(super::native_storage::NativeStorage),
            ));
            super::server::serve(engine, cancellation).await?;
            Ok(CliOutput::none())
        }
        AppRequest::Cleanup(id) => {
            super::persistence::cleanup(&id, false)?;
            Ok(CliOutput::none())
        }
        AppRequest::Control(request) => {
            let client = Client::new(None)?.with_deadlines(
                std::time::Duration::from_secs(5),
                std::time::Duration::from_secs(10),
            );
            match request.operation.as_str() {
                "status" => Ok(CliOutput::facet(client.status().await?)),
                "purge" => {
                    client.purge(request.lease_id).await?;
                    Ok(CliOutput::none())
                }
                "stop" => {
                    client.stop().await?;
                    Ok(CliOutput::none())
                }
                _ => eyre::bail!("invalid control request"),
            }
        }
    }
}
