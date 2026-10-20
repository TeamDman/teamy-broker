use super::app_request::AppRequest;
use crate::cli::output::CliOutput;
use crate::sdk::Client;
use std::sync::Arc;
use std::time::Duration;
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub async fn invoke(
    request: AppRequest,
    cancellation: teamy_cancellation::CancellationToken,
) -> eyre::Result<CliOutput> {
    cancellation.bail_if_cancelled()?;
    match request {
        AppRequest::PairingConfirm { attempt_id, code } => {
            let client = Client::new(None)?;
            let result = tokio::select! {
                result=client.pairing_confirm(&attempt_id,&code)=>result?,
                ()=async{while !cancellation.is_cancelled(){tokio::time::sleep(Duration::from_millis(20)).await;}}=>return Err(crate::protocol::FailureKind::RequestCancelled.into()),
            };
            Ok(CliOutput::facet(result, None))
        }
        AppRequest::Preview(show) => {
            if !show {
                return Ok(CliOutput::facet("PROPOSAL ONLY: preview --show-test-dialog opens one TEST/DUMMY window through the already-running resident broker. No vault, credential or lease; no autostart. Coordinate timing first.".to_string(),Some(Duration::from_secs(10))));
            }
            let client =
                Client::new(None)?.with_deadlines(Duration::from_secs(5), Duration::from_secs(70));
            let result = tokio::select! {
                result=client.approval_preview()=>result?,
                ()=async {while !cancellation.is_cancelled(){tokio::time::sleep(Duration::from_millis(50)).await;}}=>return Err(crate::protocol::FailureKind::RequestCancelled.into()),
            };
            Ok(CliOutput::facet(result, None))
        }
        AppRequest::Serve(executable) => {
            tracing::info!(phase = "engine-create", "Broker startup");
            let engine = Arc::new(super::engine::Engine::new(
                Arc::new(super::one_password::OnePassword { executable }),
                Arc::new(super::native_approval::NativeApproval),
                Arc::new(super::native_storage::NativeStorage),
            ));
            tracing::info!(phase = "engine-ready", "Broker startup");
            // Serving includes interactive approvals and runs until explicitly stopped.
            super::server::serve(engine, cancellation).await?;
            Ok(CliOutput::none(None))
        }
        AppRequest::Cleanup(id) => {
            super::persistence::cleanup(&id, false)?;
            Ok(CliOutput::none(Some(Duration::from_secs(10))))
        }
        AppRequest::Control(request) => {
            let client = Client::new(None)?.with_deadlines(
                std::time::Duration::from_secs(5),
                std::time::Duration::from_secs(10),
            );
            match request.operation.as_str() {
                "phone-authorize-desktop"
                | "phone-authorize-supply"
                | "phone-authorize-fixture" => Ok(CliOutput::facet(
                    client.phone_authorize_once(request).await?,
                    None,
                )),
                "phone-pair-inspect" => Ok(CliOutput::facet(
                    client
                        .pairing_inspect(
                            request
                                .lease_id
                                .as_deref()
                                .ok_or_else(|| eyre::eyre!("attempt required"))?,
                        )
                        .await?,
                    None,
                )),
                "phone-pair-cancel" => Ok(CliOutput::facet(
                    client
                        .pairing_cancel(
                            request
                                .lease_id
                                .as_deref()
                                .ok_or_else(|| eyre::eyre!("attempt required"))?,
                        )
                        .await?,
                    None,
                )),
                "phone-pair" | "phone-revoke" | "phone-list" | "phone-status" => Ok(
                    CliOutput::facet(client.phone_control(&request.operation).await?, None),
                ),
                "status" => Ok(CliOutput::facet(
                    client.status().await?,
                    Some(Duration::from_secs(10)),
                )),
                "purge" => {
                    client.purge(request.lease_id).await?;
                    Ok(CliOutput::none(Some(Duration::from_secs(10))))
                }
                "stop" | "stop-force" => {
                    if request.operation == "stop-force" {
                        client.stop_force().await?;
                    } else {
                        client.stop().await?;
                    }
                    Ok(CliOutput::none(Some(Duration::from_secs(10))))
                }
                _ => eyre::bail!("invalid control request"),
            }
        }
    }
}
