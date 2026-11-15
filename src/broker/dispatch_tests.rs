use super::app_request::AppRequest;
use super::app_result::AppResult;
use super::dispatch::invoke_with_client;
use super::phone_authorization_mode::PhoneAuthorizationMode;
use super::phone_authorization_request::PhoneAuthorizationRequest;
use super::purge_scope::PurgeScope;
use super::synthetic_credential_fixture;
use crate::cli::Command;
use crate::cli::output::CliOutput;
use crate::cli::output::OutputFormat;
use crate::cli::phone_pair_confirm::PhonePairConfirmArgs;
use crate::platform::pipe_security;
use crate::protocol::CAPABILITIES;
use crate::protocol::FailureKind;
use crate::protocol::Hello;
use crate::protocol::PhoneStatus;
use crate::protocol::Request;
use crate::protocol::Response;
use crate::protocol::VERSION;
use crate::protocol::pairing_comparison::PairingCode;
use crate::protocol::pairing_comparison::PairingConfirmationResult;
use crate::protocol::phone_status::PhoneConnectionState;
use crate::protocol::phone_status::PhoneDiagnostic;
use crate::protocol::phone_status::PhoneRuntimeState;
use crate::protocol::wire;
use crate::sdk::Client;
use std::time::Duration;
use teamy_cancellation::CancellationToken;
use tokio::io::AsyncReadExt;
use tokio::net::windows::named_pipe::NamedPipeServer;
use tokio::sync::oneshot;
use tokio::time;
use uuid::Uuid;

fn endpoint() -> (NamedPipeServer, Client) {
    let endpoint = format!(r"\\.\pipe\teamy-dispatch-test-{}", Uuid::new_v4());
    let pipe = pipe_security::create(&endpoint, true).unwrap();
    let client = Client::new(None).unwrap().with_endpoint(endpoint);
    (pipe, client)
}

async fn receive(pipe: &mut NamedPipeServer) -> Request {
    pipe.connect().await.unwrap();
    wire::write(
        pipe,
        &Hello {
            version: VERSION,
            instance_id: Uuid::new_v4().to_string(),
            capabilities: CAPABILITIES
                .iter()
                .copied()
                .chain(["shutdown-state-v1", "persistent-no-vault-phone-v1"])
                .map(Into::into)
                .collect(),
        },
    )
    .await
    .unwrap();
    let _: Hello = wire::read(pipe).await.unwrap();
    let request: Request = wire::read(pipe).await.unwrap();
    request.validate().unwrap();
    request
}

fn phone_status() -> PhoneStatus {
    PhoneStatus {
        paired: false,
        pairing_window_open: false,
        pairing_attempt_id: None,
        listener: false,
        connection: PhoneConnectionState::Disconnected,
        runtime: PhoneRuntimeState::Stopped,
        diagnostic: PhoneDiagnostic::default(),
        scope: "synthetic".into(),
        peer: "synthetic".into(),
    }
}

#[tokio::test]
async fn read_only_control_cancellation_closes_the_owned_exchange_promptly() {
    let id = Uuid::new_v4().to_string();
    for (request, operation) in [
        (AppRequest::Status, "status"),
        (AppRequest::PhoneStatus, "phone-status"),
        (AppRequest::PhoneList, "phone-list"),
        (
            AppRequest::PairingInspect(id.clone().try_into().unwrap()),
            "phone-pair-inspect",
        ),
    ] {
        let (mut pipe, client) = endpoint();
        let (seen, received) = oneshot::channel();
        let server = tokio::spawn(async move {
            assert_eq!(receive(&mut pipe).await.operation, operation);
            seen.send(()).unwrap();
            let mut byte = [0];
            assert_eq!(
                time::timeout(Duration::from_secs(1), pipe.read(&mut byte))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            );
        });
        let cancellation = CancellationToken::new();
        let token = cancellation.clone();
        let pending = tokio::spawn(invoke_with_client(request, token, Some(client)));
        time::timeout(Duration::from_secs(2), received)
            .await
            .unwrap()
            .unwrap();
        cancellation.request_cancel("synthetic read cancellation");
        let result = time::timeout(Duration::from_secs(1), pending)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result.unwrap_err().downcast_ref::<FailureKind>(),
            Some(&FailureKind::RequestCancelled)
        );
        server.await.unwrap();
    }
}

#[tokio::test]
async fn confirmed_mutation_is_rendered_when_cancellation_races_its_response() {
    // Uppercase remains byte-for-byte identical at the wire boundary.
    let id = Uuid::new_v4().to_string().to_uppercase();
    let command = Command::PhonePairConfirm(PhonePairConfirmArgs {
        attempt_id: id.clone(),
        comparison_stdin: true,
    });
    let policy = command.completion_policy();
    let request = command
        .into_request_with_comparison(Some(PairingCode {
            comparison: "synthetic-comparison".into(),
        }))
        .unwrap();
    let (mut pipe, client) = endpoint();
    let (seen, received) = oneshot::channel();
    let (reply, released) = oneshot::channel();
    let wire_id = id.clone();
    let server = tokio::spawn(async move {
        let request = receive(&mut pipe).await;
        assert_eq!(request.operation, "phone-pair-confirm");
        assert_eq!(request.lease_id.as_deref(), Some(wire_id.as_str()));
        let code: PairingCode = wire::read(&mut pipe).await.unwrap();
        assert_eq!(code.comparison, "synthetic-comparison");
        seen.send(()).unwrap();
        released.await.unwrap();
        wire::write(&mut pipe, &Response::code("pairing-confirmation"))
            .await
            .unwrap();
        wire::write(
            &mut pipe,
            &PairingConfirmationResult {
                attempt_id: wire_id,
                accepted: true,
            },
        )
        .await
        .unwrap();
    });
    let cancellation = CancellationToken::new();
    let mut pending = tokio::spawn(invoke_with_client(
        request,
        cancellation.clone(),
        Some(client),
    ));
    time::timeout(Duration::from_secs(2), received)
        .await
        .unwrap()
        .unwrap();
    cancellation.request_cancel("synthetic mutation cancellation");
    let premature = time::timeout(Duration::from_millis(100), &mut pending).await;
    reply.send(()).unwrap();
    let result = match premature {
        Ok(result) => {
            let _ = server.await;
            panic!("mutation completed before the resident response: {result:?}");
        }
        Err(_elapsed) => pending.await.unwrap().unwrap(),
    };
    server.await.unwrap();
    let mut bytes = Vec::new();
    CliOutput::from_app_result(result, None)
        .emit_completed_to(
            &mut bytes,
            Some(OutputFormat::Json),
            false,
            &cancellation,
            policy,
        )
        .unwrap();
    let encoded = String::from_utf8(bytes).unwrap();
    let report: PairingConfirmationResult = facet_json::from_str(&encoded).unwrap();
    assert!(report.accepted);
    assert_eq!(report.attempt_id, id);
}

#[tokio::test]
async fn cancelled_before_start_does_not_send_a_mutation() {
    let (pipe, client) = endpoint();
    let cancellation = CancellationToken::new();
    cancellation.request_cancel("synthetic pre-start cancellation");
    let result = invoke_with_client(AppRequest::PhoneRevoke, cancellation, Some(client)).await;
    assert_eq!(
        result.unwrap_err().to_string(),
        "synthetic pre-start cancellation"
    );
    let _ = time::timeout(Duration::from_millis(30), pipe.connect())
        .await
        .expect_err("pre-start cancellation must leave the pipe unconnected");
}

#[tokio::test]
async fn failed_mutation_response_is_not_promoted_to_success_after_cancellation() {
    let (mut pipe, client) = endpoint();
    let cancellation = CancellationToken::new();
    let trigger = cancellation.clone();
    let server = tokio::spawn(async move {
        assert_eq!(receive(&mut pipe).await.operation, "phone-revoke");
        trigger.request_cancel("synthetic cancellation after send");
        wire::write(&mut pipe, &Response::code("failed"))
            .await
            .unwrap();
    });
    let result = invoke_with_client(AppRequest::PhoneRevoke, cancellation, Some(client)).await;
    assert_eq!(
        result.unwrap_err().to_string(),
        "phone control rejected; inspect status and revoke before replacing"
    );
    server.await.unwrap();
}

#[tokio::test]
async fn typed_control_requests_preserve_existing_wire_operations_and_scopes() {
    let id = Uuid::new_v4().to_string().to_uppercase();
    let mut cases = vec![
        (AppRequest::Purge(PurgeScope::All), "purge", None),
        (
            AppRequest::Purge(PurgeScope::Lease(id.clone().try_into().unwrap())),
            "purge",
            Some(id.clone()),
        ),
        (AppRequest::StopWhenIdle, "stop", None),
        (AppRequest::StopForce, "stop-force", None),
        (AppRequest::PhonePair, "phone-pair", None),
        (AppRequest::PhoneRevoke, "phone-revoke", None),
        (
            AppRequest::PairingCancel(id.clone().try_into().unwrap()),
            "phone-pair-cancel",
            Some(id),
        ),
    ];
    for (mode, operation) in [
        (
            PhoneAuthorizationMode::DesktopOnly,
            "phone-authorize-desktop",
        ),
        (
            PhoneAuthorizationMode::AllowSupply,
            "phone-authorize-supply",
        ),
        (
            PhoneAuthorizationMode::SyntheticFixture,
            "phone-authorize-fixture",
        ),
    ] {
        cases.push((
            AppRequest::PhoneAuthorize(
                PhoneAuthorizationRequest::new(synthetic_credential_fixture::request(), mode)
                    .unwrap(),
            ),
            operation,
            None,
        ));
    }
    for (request, operation, expected_id) in cases {
        let (mut pipe, client) = endpoint();
        let server = tokio::spawn(async move {
            let request = receive(&mut pipe).await;
            assert_eq!(request.operation, operation);
            assert_eq!(request.lease_id, expected_id);
            if operation.starts_with("phone-authorize-") {
                assert_eq!(
                    request.acquire,
                    Some(synthetic_credential_fixture::request())
                );
            } else {
                assert!(request.acquire.is_none());
            }
            if operation == "phone-pair-cancel" {
                wire::write(&mut pipe, &Response::code("pairing-confirmation"))
                    .await
                    .unwrap();
                wire::write(
                    &mut pipe,
                    &PairingConfirmationResult {
                        attempt_id: expected_id.unwrap(),
                        accepted: true,
                    },
                )
                .await
                .unwrap();
            } else if operation.starts_with("phone-") {
                wire::write(&mut pipe, &Response::code("phone-status"))
                    .await
                    .unwrap();
                wire::write(&mut pipe, &phone_status()).await.unwrap();
            } else {
                wire::write(&mut pipe, &Response::code("ok")).await.unwrap();
            }
        });
        let result = time::timeout(
            Duration::from_secs(2),
            invoke_with_client(request, CancellationToken::new(), Some(client)),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(matches!(
            result,
            AppResult::Completed | AppResult::PhoneStatus(_) | AppResult::PairingConfirmation(_)
        ));
        server.await.unwrap();
    }
}
