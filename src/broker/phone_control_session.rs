use crate::protocol::Hello;
use crate::protocol::Request;
use crate::protocol::Response;
use crate::protocol::{self};
use tokio::net::windows::named_pipe::NamedPipeServer;
pub(super) async fn execute(
    pipe: &mut NamedPipeServer,
    peer: &Hello,
    request: &Request,
) -> eyre::Result<()> {
    let controller = super::phone::Controller::resident()?;
    execute_with(pipe, peer, request, &controller).await
}
pub(crate) async fn execute_with(
    pipe: &mut NamedPipeServer,
    peer: &Hello,
    request: &Request,
    controller: &super::phone::Controller,
) -> eyre::Result<()> {
    eyre::ensure!(
        peer.capabilities
            .iter()
            .any(|cap| cap == "persistent-no-vault-phone-v1"),
        "phone capability required"
    );

    if matches!(
        request.operation.as_str(),
        "phone-pair-inspect" | "phone-pair-confirm" | "phone-pair-cancel"
    ) {
        let id = request
            .lease_id
            .as_deref()
            .ok_or_else(|| eyre::eyre!("attempt ID required"))?;
        let attempt = controller.pairing_attempt(id)?;
        if request.operation == "phone-pair-inspect" {
            let snapshot = attempt.inspect(id)?;
            protocol::wire::write(pipe, &Response::code("pairing-comparison")).await?;
            return protocol::wire::write(pipe, &snapshot).await;
        }
        let accepted = if request.operation == "phone-pair-confirm" {
            let code: protocol::pairing_comparison::PairingCode = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                protocol::wire::read(pipe),
            )
            .await??;
            eyre::ensure!(code.comparison.len() <= 128, "comparison rejected");
            attempt.confirm(id, &code.comparison)?
        } else {
            attempt.cancel_pending(id)?;
            false
        };
        protocol::wire::write(pipe, &Response::code("pairing-confirmation")).await?;
        return protocol::wire::write(
            pipe,
            &protocol::pairing_comparison::PairingConfirmationResult {
                attempt_id: id.into(),
                accepted,
            },
        )
        .await;
    }
    let result = match request.operation.as_str() {
        "phone-authorize-desktop" | "phone-authorize-supply" => controller.authorize_once(
            request
                .acquire
                .clone()
                .ok_or_else(|| eyre::eyre!("missing exact permission metadata"))?,
            request.operation == "phone-authorize-supply",
        ),
        "phone-authorize-fixture" => controller.authorize_fixture(
            request
                .acquire
                .clone()
                .ok_or_else(|| eyre::eyre!("missing fixture metadata"))?,
        ),
        "phone-pair" => controller.arm(),
        "phone-revoke" => controller.revoke(),
        _ => Ok(()),
    };
    protocol::wire::write(
        pipe,
        &Response::code(if result.is_ok() {
            "phone-status"
        } else {
            "phone-control-rejected"
        }),
    )
    .await?;
    if result.is_ok() {
        protocol::wire::write(pipe, &controller.status()?).await?;
    }
    Ok(())
}
