use super::app_request::AppRequest;
use super::app_result::AppResult;
use super::phone_authorization_mode::PhoneAuthorizationMode;
use super::purge_scope::PurgeScope;
use crate::protocol::FailureKind;
use crate::protocol::Request;
use crate::sdk::Client;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use teamy_cancellation::CancellationToken;
use tokio::time;

/// # Errors
/// Returns validation, cancellation, provider, transport or persistence failures.
pub async fn invoke(
    request: AppRequest,
    cancellation: CancellationToken,
) -> eyre::Result<AppResult> {
    invoke_with_client(request, cancellation, None).await
}

pub(crate) async fn invoke_with_client(
    request: AppRequest,
    cancellation: CancellationToken,
    supplied_client: Option<Client>,
) -> eyre::Result<AppResult> {
    cancellation.bail_if_cancelled()?;
    match request {
        AppRequest::PhoneConfig(request) => Ok(AppResult::PhoneConfig(request.invoke()?)),
        AppRequest::Serve { op_executable } => serve(op_executable, cancellation).await,
        AppRequest::Cleanup(id) => {
            super::persistence::cleanup(id.as_str(), false)?;
            Ok(AppResult::Completed)
        }
        AppRequest::PreviewProposal => Ok(AppResult::PreviewProposal),
        AppRequest::ApprovalPreview => {
            let client = control_client(supplied_client)?
                .with_deadlines(Duration::from_secs(5), Duration::from_secs(70));
            // Closing this request's pipe cancels its ephemeral dialog on the resident.
            await_cancellable(client.approval_preview(), &cancellation)
                .await
                .map(AppResult::ApprovalPreview)
        }
        AppRequest::Status => {
            let client = control_client(supplied_client)?;
            await_cancellable(client.status(), &cancellation)
                .await
                .map(AppResult::Status)
        }
        AppRequest::PhoneStatus | AppRequest::PhoneList => {
            let operation = if matches!(request, AppRequest::PhoneStatus) {
                "phone-status"
            } else {
                "phone-list"
            };
            let client = control_client(supplied_client)?;
            await_cancellable(client.phone_control(operation), &cancellation)
                .await
                .map(AppResult::PhoneStatus)
        }
        AppRequest::PairingInspect(attempt_id) => {
            let client = control_client(supplied_client)?;
            await_cancellable(client.pairing_inspect(attempt_id.as_str()), &cancellation)
                .await
                .map(AppResult::PairingInspection)
        }
        // Mutations retain their bounded exchange until its response is known. Dropping
        // the future cannot undo a request already written; never retry these controls.
        AppRequest::PhonePair | AppRequest::PhoneRevoke => {
            let operation = if matches!(request, AppRequest::PhonePair) {
                "phone-pair"
            } else {
                "phone-revoke"
            };
            control_client(supplied_client)?
                .phone_control(operation)
                .await
                .map(AppResult::PhoneStatus)
        }
        AppRequest::PhoneAuthorize(request) => {
            let (acquire, mode) = request.into_parts();
            let operation = match mode {
                PhoneAuthorizationMode::DesktopOnly => "phone-authorize-desktop",
                PhoneAuthorizationMode::AllowSupply => "phone-authorize-supply",
                PhoneAuthorizationMode::SyntheticFixture => "phone-authorize-fixture",
            };
            let wire_request = Request {
                operation: operation.into(),
                acquire: Some(acquire),
                lease_id: None,
            };
            control_client(supplied_client)?
                .phone_authorize_once(wire_request)
                .await
                .map(AppResult::PhoneStatus)
        }
        AppRequest::PairingCancel(attempt_id) => control_client(supplied_client)?
            .pairing_cancel(attempt_id.as_str())
            .await
            .map(AppResult::PairingConfirmation),
        AppRequest::PairingConfirm { attempt_id, code } => control_client(supplied_client)?
            .with_deadlines(Duration::from_secs(5), Duration::from_secs(5))
            .pairing_confirm(attempt_id.as_str(), &code)
            .await
            .map(AppResult::PairingConfirmation),
        AppRequest::Purge(scope) => {
            let wire_id = match scope {
                PurgeScope::All => None,
                PurgeScope::Lease(id) => Some(id.into_string()),
            };
            control_client(supplied_client)?.purge(wire_id).await?;
            Ok(AppResult::Completed)
        }
        AppRequest::StopWhenIdle | AppRequest::StopForce => {
            let client = control_client(supplied_client)?;
            if matches!(request, AppRequest::StopForce) {
                client.stop_force().await?;
            } else {
                client.stop().await?;
            }
            Ok(AppResult::Completed)
        }
    }
}

async fn serve(op_executable: PathBuf, cancellation: CancellationToken) -> eyre::Result<AppResult> {
    eyre::ensure!(
        !op_executable.as_os_str().is_empty(),
        "provider executable required"
    );
    tracing::info!(phase = "engine-create", "Broker startup");
    let engine = Arc::new(super::engine::Engine::new(
        Arc::new(super::one_password::OnePassword {
            executable: op_executable,
        }),
        Arc::new(super::native_approval::NativeApproval),
        Arc::new(super::native_storage::NativeStorage),
    ));
    tracing::info!(phase = "engine-ready", "Broker startup");
    super::server::serve(engine, cancellation).await?;
    Ok(AppResult::Completed)
}

fn control_client(supplied: Option<Client>) -> eyre::Result<Client> {
    Ok(supplied
        .map_or_else(|| Client::new(None), Ok)?
        .with_deadlines(Duration::from_secs(5), Duration::from_secs(10)))
}

async fn await_cancellable<T>(
    operation: impl Future<Output = eyre::Result<T>>,
    cancellation: &CancellationToken,
) -> eyre::Result<T> {
    tokio::select! {
        biased;
        () = async {
            while !cancellation.is_cancelled() {
                time::sleep(Duration::from_millis(20)).await;
            }
        } => Err(FailureKind::RequestCancelled.into()),
        result = operation => result,
    }
}
